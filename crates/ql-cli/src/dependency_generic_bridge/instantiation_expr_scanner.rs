use std::collections::BTreeSet;

use ql_ast::{self, Expr, ExprKind, FunctionDecl, TypeExpr};

use super::call_inference::{
    call_arg_expr, infer_dependency_generic_function_substitutions, ordered_call_arg_expected_types,
};
use super::expr_inference::ValueTypeBindings;
use super::function_bindings::FunctionTypeBindings;
use super::instantiation_scanner::{
    PublicFunctionCallInstantiation, collect_dependency_generic_function_instantiations_from_block,
};

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
    match &expr.kind {
        ExprKind::Call { callee, args } => {
            let ordered_arg_expected_types = ordered_call_arg_expected_types(
                callee,
                args,
                expected_ty,
                bindings,
                function_bindings,
            );
            if let ExprKind::Name(name) = &callee.kind
                && local_names.contains(name)
            {
                *saw_call = true;
                if let Some(substitutions) = infer_dependency_generic_function_substitutions(
                    function,
                    args,
                    expected_ty,
                    bindings,
                    function_bindings,
                ) {
                    instantiations.push(PublicFunctionCallInstantiation {
                        callee_span: callee.span,
                        substitutions,
                    });
                }
            }
            collect_dependency_generic_function_instantiations_from_expr(
                callee,
                None,
                return_expected_ty,
                local_names,
                function,
                bindings,
                function_bindings,
                saw_call,
                instantiations,
            );
            for (arg, arg_expected_ty) in args.iter().zip(ordered_arg_expected_types.iter()) {
                collect_dependency_generic_function_instantiations_from_expr(
                    call_arg_expr(arg),
                    arg_expected_ty.as_ref(),
                    return_expected_ty,
                    local_names,
                    function,
                    bindings,
                    function_bindings,
                    saw_call,
                    instantiations,
                );
            }
        }
        ExprKind::Tuple(items) | ExprKind::Array(items) => {
            for item in items {
                collect_dependency_generic_function_instantiations_from_expr(
                    item,
                    None,
                    return_expected_ty,
                    local_names,
                    function,
                    bindings,
                    function_bindings,
                    saw_call,
                    instantiations,
                );
            }
        }
        ExprKind::RepeatArray { value, .. } => {
            collect_dependency_generic_function_instantiations_from_expr(
                value,
                None,
                return_expected_ty,
                local_names,
                function,
                bindings,
                function_bindings,
                saw_call,
                instantiations,
            );
        }
        ExprKind::StructLiteral { fields, .. } => {
            for field in fields {
                if let Some(value) = &field.value {
                    collect_dependency_generic_function_instantiations_from_expr(
                        value,
                        None,
                        return_expected_ty,
                        local_names,
                        function,
                        bindings,
                        function_bindings,
                        saw_call,
                        instantiations,
                    );
                }
            }
        }
        ExprKind::Binary { left, right, .. } => {
            collect_dependency_generic_function_instantiations_from_expr(
                left,
                None,
                return_expected_ty,
                local_names,
                function,
                bindings,
                function_bindings,
                saw_call,
                instantiations,
            );
            collect_dependency_generic_function_instantiations_from_expr(
                right,
                None,
                return_expected_ty,
                local_names,
                function,
                bindings,
                function_bindings,
                saw_call,
                instantiations,
            );
        }
        ExprKind::Unary { expr, .. } | ExprKind::Question(expr) => {
            collect_dependency_generic_function_instantiations_from_expr(
                expr,
                None,
                return_expected_ty,
                local_names,
                function,
                bindings,
                function_bindings,
                saw_call,
                instantiations,
            );
        }
        ExprKind::Member { object, .. } => {
            collect_dependency_generic_function_instantiations_from_expr(
                object,
                None,
                return_expected_ty,
                local_names,
                function,
                bindings,
                function_bindings,
                saw_call,
                instantiations,
            );
        }
        ExprKind::Bracket { target, items } => {
            collect_dependency_generic_function_instantiations_from_expr(
                target,
                None,
                return_expected_ty,
                local_names,
                function,
                bindings,
                function_bindings,
                saw_call,
                instantiations,
            );
            for item in items {
                collect_dependency_generic_function_instantiations_from_expr(
                    item,
                    None,
                    return_expected_ty,
                    local_names,
                    function,
                    bindings,
                    function_bindings,
                    saw_call,
                    instantiations,
                );
            }
        }
        ExprKind::Block(block) | ExprKind::Unsafe(block) => {
            let mut block_bindings = bindings.clone();
            collect_dependency_generic_function_instantiations_from_block(
                block,
                local_names,
                function,
                &mut block_bindings,
                function_bindings,
                saw_call,
                instantiations,
                return_expected_ty,
                expected_ty,
            );
        }
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_dependency_generic_function_instantiations_from_expr(
                condition,
                None,
                return_expected_ty,
                local_names,
                function,
                bindings,
                function_bindings,
                saw_call,
                instantiations,
            );
            let mut then_bindings = bindings.clone();
            collect_dependency_generic_function_instantiations_from_block(
                then_branch,
                local_names,
                function,
                &mut then_bindings,
                function_bindings,
                saw_call,
                instantiations,
                return_expected_ty,
                expected_ty,
            );
            if let Some(else_branch) = else_branch {
                collect_dependency_generic_function_instantiations_from_expr(
                    else_branch,
                    expected_ty,
                    return_expected_ty,
                    local_names,
                    function,
                    bindings,
                    function_bindings,
                    saw_call,
                    instantiations,
                );
            }
        }
        ExprKind::Match { value, arms } => {
            collect_dependency_generic_function_instantiations_from_expr(
                value,
                None,
                return_expected_ty,
                local_names,
                function,
                bindings,
                function_bindings,
                saw_call,
                instantiations,
            );
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    collect_dependency_generic_function_instantiations_from_expr(
                        guard,
                        None,
                        return_expected_ty,
                        local_names,
                        function,
                        bindings,
                        function_bindings,
                        saw_call,
                        instantiations,
                    );
                }
                collect_dependency_generic_function_instantiations_from_expr(
                    &arm.body,
                    expected_ty,
                    return_expected_ty,
                    local_names,
                    function,
                    bindings,
                    function_bindings,
                    saw_call,
                    instantiations,
                );
            }
        }
        ExprKind::Closure { body, .. } => {
            collect_dependency_generic_function_instantiations_from_expr(
                body,
                None,
                None,
                local_names,
                function,
                bindings,
                function_bindings,
                saw_call,
                instantiations,
            );
        }
        ExprKind::Integer(_)
        | ExprKind::String { .. }
        | ExprKind::Bool(_)
        | ExprKind::NoneLiteral
        | ExprKind::Name(_) => {}
    }
}
