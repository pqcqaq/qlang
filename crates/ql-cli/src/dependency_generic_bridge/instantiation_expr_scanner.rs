use std::collections::BTreeSet;

use ql_ast::{self, CallArg, Expr, ExprKind, FunctionDecl, TypeExpr};

use super::call_args::{call_arg_expr, ordered_call_arg_expected_types};
use super::call_inference::infer_dependency_generic_function_substitutions;
use super::function_bindings::FunctionTypeBindings;
use super::instantiation_scanner::{
    PublicFunctionCallInstantiation, collect_dependency_generic_function_instantiations_from_block,
};
use super::value_bindings::ValueTypeBindings;

pub(super) fn collect_dependency_generic_function_instantiations_from_expr(
    expr: &Expr,
    expected_ty: Option<&TypeExpr>,
    return_expected_ty: Option<&TypeExpr>,
    local_names: &BTreeSet<String>,
    function: &FunctionDecl,
    bindings: &ValueTypeBindings,
    function_bindings: &FunctionTypeBindings,
    saw_call: &mut bool,
    instantiations: &mut Vec<PublicFunctionCallInstantiation>,
) {
    let mut scanner = ExprInstantiationScanner {
        local_names,
        function,
        function_bindings,
        saw_call,
        instantiations,
    };
    scanner.scan_expr(expr, expected_ty, return_expected_ty, bindings);
}

struct ExprInstantiationScanner<'a, 'out> {
    local_names: &'a BTreeSet<String>,
    function: &'a FunctionDecl,
    function_bindings: &'a FunctionTypeBindings,
    saw_call: &'out mut bool,
    instantiations: &'out mut Vec<PublicFunctionCallInstantiation>,
}

impl ExprInstantiationScanner<'_, '_> {
    fn scan_expr(
        &mut self,
        expr: &Expr,
        expected_ty: Option<&TypeExpr>,
        return_expected_ty: Option<&TypeExpr>,
        bindings: &ValueTypeBindings,
    ) {
        match &expr.kind {
            ExprKind::Call { callee, args } => {
                self.scan_call_expr(callee, args, expected_ty, return_expected_ty, bindings);
            }
            ExprKind::Tuple(items) | ExprKind::Array(items) => {
                self.scan_exprs_without_expected(items, return_expected_ty, bindings);
            }
            ExprKind::RepeatArray { value, .. } => {
                self.scan_child_expr(value, return_expected_ty, bindings);
            }
            ExprKind::StructLiteral { fields, .. } => {
                for field in fields {
                    if let Some(value) = &field.value {
                        self.scan_child_expr(value, return_expected_ty, bindings);
                    }
                }
            }
            ExprKind::Binary { left, right, .. } => {
                self.scan_child_expr(left, return_expected_ty, bindings);
                self.scan_child_expr(right, return_expected_ty, bindings);
            }
            ExprKind::Unary { expr, .. } | ExprKind::Question(expr) => {
                self.scan_child_expr(expr, return_expected_ty, bindings);
            }
            ExprKind::Member { object, .. } => {
                self.scan_child_expr(object, return_expected_ty, bindings);
            }
            ExprKind::Bracket { target, items } => {
                self.scan_child_expr(target, return_expected_ty, bindings);
                self.scan_exprs_without_expected(items, return_expected_ty, bindings);
            }
            ExprKind::Block(block) | ExprKind::Unsafe(block) => {
                self.scan_block_expr(block, expected_ty, return_expected_ty, bindings);
            }
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.scan_if_expr(
                    condition,
                    then_branch,
                    else_branch.as_deref(),
                    expected_ty,
                    return_expected_ty,
                    bindings,
                );
            }
            ExprKind::Match { value, arms } => {
                self.scan_child_expr(value, return_expected_ty, bindings);
                for arm in arms {
                    if let Some(guard) = &arm.guard {
                        self.scan_child_expr(guard, return_expected_ty, bindings);
                    }
                    self.scan_expr(&arm.body, expected_ty, return_expected_ty, bindings);
                }
            }
            ExprKind::Closure { body, .. } => {
                self.scan_expr(body, None, None, bindings);
            }
            ExprKind::Integer(_)
            | ExprKind::String { .. }
            | ExprKind::Bool(_)
            | ExprKind::NoneLiteral
            | ExprKind::Name(_) => {}
        }
    }

    fn scan_call_expr(
        &mut self,
        callee: &Expr,
        args: &[CallArg],
        expected_ty: Option<&TypeExpr>,
        return_expected_ty: Option<&TypeExpr>,
        bindings: &ValueTypeBindings,
    ) {
        let ordered_arg_expected_types = ordered_call_arg_expected_types(
            callee,
            args,
            expected_ty,
            bindings,
            self.function_bindings,
        );
        if let ExprKind::Name(name) = &callee.kind
            && self.local_names.contains(name)
        {
            *self.saw_call = true;
            if let Some(substitutions) = infer_dependency_generic_function_substitutions(
                self.function,
                args,
                expected_ty,
                bindings,
                self.function_bindings,
            ) {
                self.instantiations.push(PublicFunctionCallInstantiation {
                    callee_span: callee.span,
                    substitutions,
                });
            }
        }
        self.scan_child_expr(callee, return_expected_ty, bindings);
        for (arg, arg_expected_ty) in args.iter().zip(ordered_arg_expected_types.iter()) {
            self.scan_expr(
                call_arg_expr(arg),
                arg_expected_ty.as_ref(),
                return_expected_ty,
                bindings,
            );
        }
    }

    fn scan_if_expr(
        &mut self,
        condition: &Expr,
        then_branch: &ql_ast::Block,
        else_branch: Option<&Expr>,
        expected_ty: Option<&TypeExpr>,
        return_expected_ty: Option<&TypeExpr>,
        bindings: &ValueTypeBindings,
    ) {
        self.scan_child_expr(condition, return_expected_ty, bindings);
        self.scan_block_expr(then_branch, expected_ty, return_expected_ty, bindings);
        if let Some(else_branch) = else_branch {
            self.scan_expr(else_branch, expected_ty, return_expected_ty, bindings);
        }
    }

    fn scan_block_expr(
        &mut self,
        block: &ql_ast::Block,
        tail_expected_ty: Option<&TypeExpr>,
        return_expected_ty: Option<&TypeExpr>,
        bindings: &ValueTypeBindings,
    ) {
        let mut block_bindings = bindings.clone();
        collect_dependency_generic_function_instantiations_from_block(
            block,
            self.local_names,
            self.function,
            &mut block_bindings,
            self.function_bindings,
            self.saw_call,
            self.instantiations,
            return_expected_ty,
            tail_expected_ty,
        );
    }

    fn scan_exprs_without_expected(
        &mut self,
        exprs: &[Expr],
        return_expected_ty: Option<&TypeExpr>,
        bindings: &ValueTypeBindings,
    ) {
        for expr in exprs {
            self.scan_child_expr(expr, return_expected_ty, bindings);
        }
    }

    fn scan_child_expr(
        &mut self,
        expr: &Expr,
        return_expected_ty: Option<&TypeExpr>,
        bindings: &ValueTypeBindings,
    ) {
        self.scan_expr(expr, None, return_expected_ty, bindings);
    }
}
