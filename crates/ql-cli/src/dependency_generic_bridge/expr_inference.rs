use ql_ast::{self, BinaryOp, CallArg, Expr, ExprKind, UnaryOp};

use super::call_inference::infer_function_call_return_type;
use super::function_bindings::FunctionTypeBindings;
use super::inferred_type_predicates::{
    are_inferred_bool_types, is_inferred_bool_type, is_inferred_equality_comparable_type,
    is_inferred_numeric_type, is_inferred_ordered_comparable_type,
};
use super::inferred_types::{InferredType, InferredTypeKind, render_inferred_tuple_type};
use super::value_bindings::{ValueTypeBindings, record_let_type_bindings};

pub(super) fn infer_dependency_generic_expr_type(
    expr: &Expr,
    bindings: &ValueTypeBindings,
    function_bindings: &FunctionTypeBindings,
) -> Option<InferredType> {
    ExprTypeInferencer::new(bindings, function_bindings).infer_expr(expr)
}

struct ExprTypeInferencer<'a> {
    bindings: &'a ValueTypeBindings,
    function_bindings: &'a FunctionTypeBindings,
}

impl<'a> ExprTypeInferencer<'a> {
    fn new(bindings: &'a ValueTypeBindings, function_bindings: &'a FunctionTypeBindings) -> Self {
        Self {
            bindings,
            function_bindings,
        }
    }

    fn infer_expr(&self, expr: &Expr) -> Option<InferredType> {
        match &expr.kind {
            ExprKind::Integer(_) => Some(InferredType::primitive("Int")),
            ExprKind::Bool(_) => Some(InferredType::primitive("Bool")),
            ExprKind::String { .. } => Some(InferredType::primitive("String")),
            ExprKind::Tuple(items) => self.infer_tuple_type(items),
            ExprKind::Array(items) => self.infer_array_type(items),
            ExprKind::RepeatArray { value, len, .. } => self.infer_repeat_array_type(value, len),
            ExprKind::Name(name) => self.bindings.get(name).cloned(),
            ExprKind::Block(block) | ExprKind::Unsafe(block) => self.infer_block_type(block),
            ExprKind::If {
                then_branch,
                else_branch,
                ..
            } => self.infer_if_type(then_branch, else_branch.as_deref()),
            ExprKind::Match { arms, .. } => self.infer_match_type(arms),
            ExprKind::Call { callee, args } => self.infer_call_type(callee, args),
            ExprKind::Bracket { target, items } => self.infer_projection_type(target, items),
            ExprKind::Binary { left, op, right } => self.infer_binary_expr_type(left, *op, right),
            ExprKind::Unary { op, expr } => self.infer_unary_expr_type(*op, expr),
            _ => None,
        }
    }

    fn infer_tuple_type(&self, items: &[Expr]) -> Option<InferredType> {
        let items = items
            .iter()
            .map(|item| self.infer_expr(item))
            .collect::<Option<Vec<_>>>()?;
        Some(InferredType {
            rendered: render_inferred_tuple_type(&items),
            kind: InferredTypeKind::Tuple(items),
        })
    }

    fn infer_array_type(&self, items: &[Expr]) -> Option<InferredType> {
        let (first, rest) = items.split_first()?;
        let element = self.infer_expr(first)?;
        for item in rest {
            let item_ty = self.infer_expr(item)?;
            if item_ty != element {
                return None;
            }
        }
        Some(InferredType {
            rendered: format!("[{}; {}]", element.rendered, items.len()),
            kind: InferredTypeKind::Array {
                element: Box::new(element),
                len: items.len().to_string(),
            },
        })
    }

    fn infer_repeat_array_type(&self, value: &Expr, len: &str) -> Option<InferredType> {
        let element = self.infer_expr(value)?;
        Some(InferredType {
            rendered: format!("[{}; {len}]", element.rendered),
            kind: InferredTypeKind::Array {
                element: Box::new(element),
                len: len.to_owned(),
            },
        })
    }

    fn infer_block_type(&self, block: &ql_ast::Block) -> Option<InferredType> {
        let mut block_bindings = self.bindings.clone();
        for statement in &block.statements {
            if let ql_ast::StmtKind::Let {
                pattern, ty, value, ..
            } = &statement.kind
            {
                record_let_type_bindings(
                    pattern,
                    ty.as_ref(),
                    value,
                    &mut block_bindings,
                    self.function_bindings,
                );
            }
        }
        infer_dependency_generic_expr_type(
            block.tail.as_deref()?,
            &block_bindings,
            self.function_bindings,
        )
    }

    fn infer_if_type(
        &self,
        then_branch: &ql_ast::Block,
        else_branch: Option<&Expr>,
    ) -> Option<InferredType> {
        let then_ty = self.infer_block_type(then_branch)?;
        let else_ty = self.infer_expr(else_branch?)?;
        (then_ty == else_ty).then_some(then_ty)
    }

    fn infer_match_type(&self, arms: &[ql_ast::MatchArm]) -> Option<InferredType> {
        let (first, rest) = arms.split_first()?;
        let first_ty = self.infer_expr(&first.body)?;
        for arm in rest {
            let arm_ty = self.infer_expr(&arm.body)?;
            if arm_ty != first_ty {
                return None;
            }
        }
        Some(first_ty)
    }

    fn infer_projection_type(&self, target: &Expr, items: &[Expr]) -> Option<InferredType> {
        let [index] = items else {
            return None;
        };
        let target_ty = self.infer_expr(target)?;
        match target_ty.kind {
            InferredTypeKind::Array { element, .. } => {
                let index_ty = self.infer_expr(index)?;
                is_inferred_numeric_type(&index_ty).then_some(*element)
            }
            InferredTypeKind::Tuple(items) => {
                let ExprKind::Integer(index) = &index.kind else {
                    return None;
                };
                items.get(ql_ast::parse_usize_literal(index)?).cloned()
            }
            _ => None,
        }
    }

    fn infer_call_type(&self, callee: &Expr, args: &[CallArg]) -> Option<InferredType> {
        self.infer_single_field_generic_variant_call_type(callee, args)
            .or_else(|| {
                infer_function_call_return_type(callee, args, self.bindings, self.function_bindings)
            })
    }

    fn infer_binary_expr_type(
        &self,
        left: &Expr,
        op: BinaryOp,
        right: &Expr,
    ) -> Option<InferredType> {
        let left = self.infer_expr(left)?;
        let right = self.infer_expr(right)?;
        match op {
            BinaryOp::OrOr | BinaryOp::AndAnd if are_inferred_bool_types(&left, &right) => {
                Some(InferredType::primitive("Bool"))
            }
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem
                if left == right && is_inferred_numeric_type(&left) =>
            {
                Some(left)
            }
            BinaryOp::EqEq | BinaryOp::BangEq
                if left == right && is_inferred_equality_comparable_type(&left) =>
            {
                Some(InferredType::primitive("Bool"))
            }
            BinaryOp::Gt | BinaryOp::GtEq | BinaryOp::Lt | BinaryOp::LtEq
                if left == right && is_inferred_ordered_comparable_type(&left) =>
            {
                Some(InferredType::primitive("Bool"))
            }
            BinaryOp::Assign => None,
            _ => None,
        }
    }

    fn infer_unary_expr_type(&self, op: UnaryOp, expr: &Expr) -> Option<InferredType> {
        let expr = self.infer_expr(expr)?;
        match op {
            UnaryOp::Not if is_inferred_bool_type(&expr) => Some(InferredType::primitive("Bool")),
            UnaryOp::Neg if is_inferred_numeric_type(&expr) => Some(expr),
            UnaryOp::Await | UnaryOp::Spawn => None,
            _ => None,
        }
    }

    fn infer_single_field_generic_variant_call_type(
        &self,
        callee: &Expr,
        args: &[CallArg],
    ) -> Option<InferredType> {
        let ExprKind::Member { object, .. } = &callee.kind else {
            return None;
        };
        let ExprKind::Name(type_name) = &object.kind else {
            return None;
        };
        let [CallArg::Positional(value)] = args else {
            return None;
        };
        let arg_ty = self.infer_expr(value)?;
        Some(InferredType {
            rendered: format!("{type_name}[{}]", arg_ty.rendered),
            kind: InferredTypeKind::Named {
                path: vec![type_name.clone()],
                args: vec![arg_ty],
            },
        })
    }
}

#[cfg(test)]
#[path = "expr_inference_tests.rs"]
mod tests;
