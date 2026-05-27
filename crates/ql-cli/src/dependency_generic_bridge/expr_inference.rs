use std::collections::BTreeMap;

use ql_ast::{
    self, BinaryOp, CallArg, Expr, ExprKind, FunctionDecl, Module, Param, Pattern, PatternKind,
    TypeExpr, TypeExprKind, UnaryOp,
};

use super::call_inference::infer_function_call_return_type;
use super::function_bindings::FunctionTypeBindings;
use super::inferred_type_conversion::inferred_type_from_type_expr_with_substitutions;
use super::inferred_type_predicates::{
    are_inferred_bool_types, is_inferred_bool_type, is_inferred_equality_comparable_type,
    is_inferred_numeric_type, is_inferred_ordered_comparable_type,
};
use super::inferred_types::{InferredType, InferredTypeKind, render_inferred_tuple_type};
use super::substitutions::TypeSubstitutions;

pub(super) type ValueTypeBindings = BTreeMap<String, InferredType>;

pub(super) fn collect_root_value_type_bindings(root_module: &Module) -> ValueTypeBindings {
    let mut bindings = ValueTypeBindings::new();
    for item in &root_module.items {
        let (ql_ast::ItemKind::Const(global) | ql_ast::ItemKind::Static(global)) = &item.kind
        else {
            continue;
        };
        if let Some(ty) = InferredType::from_type_expr(&global.ty) {
            bindings.insert(global.name.clone(), ty);
        }
    }
    bindings
}

pub(super) fn collect_function_param_type_bindings(
    function: &FunctionDecl,
    bindings: &mut ValueTypeBindings,
) {
    for param in &function.params {
        let Param::Regular { name, ty, .. } = param else {
            continue;
        };
        if let Some(ty) = InferredType::from_type_expr(ty) {
            bindings.insert(name.clone(), ty);
        }
    }
}

pub(super) fn collect_function_param_type_bindings_with_substitutions(
    function: &FunctionDecl,
    substitutions: &TypeSubstitutions,
    bindings: &mut ValueTypeBindings,
) {
    for param in &function.params {
        let Param::Regular { name, ty, .. } = param else {
            continue;
        };
        if let Some(ty) = inferred_type_from_type_expr_with_substitutions(ty, substitutions) {
            bindings.insert(name.clone(), ty);
        }
    }
}

pub(super) fn record_let_type_bindings(
    pattern: &Pattern,
    ty: Option<&TypeExpr>,
    value: &Expr,
    bindings: &mut ValueTypeBindings,
    function_bindings: &FunctionTypeBindings,
) {
    if let Some(ty) = ty {
        record_pattern_type_bindings(pattern, ty, bindings);
        return;
    }
    if let PatternKind::Name(name) = &pattern.kind
        && let Some(ty) = infer_dependency_generic_expr_type(value, bindings, function_bindings)
    {
        bindings.insert(name.clone(), ty);
    }
}

fn record_pattern_type_bindings(
    pattern: &Pattern,
    ty: &TypeExpr,
    bindings: &mut ValueTypeBindings,
) {
    match (&pattern.kind, &ty.kind) {
        (PatternKind::Name(name), _) => {
            if let Some(ty) = InferredType::from_type_expr(ty) {
                bindings.insert(name.clone(), ty);
            }
        }
        (PatternKind::Tuple(patterns), TypeExprKind::Tuple(types))
            if patterns.len() == types.len() =>
        {
            for (pattern, ty) in patterns.iter().zip(types) {
                record_pattern_type_bindings(pattern, ty, bindings);
            }
        }
        _ => {}
    }
}

pub(super) fn infer_dependency_generic_expr_type(
    expr: &Expr,
    bindings: &ValueTypeBindings,
    function_bindings: &FunctionTypeBindings,
) -> Option<InferredType> {
    match &expr.kind {
        ExprKind::Integer(_) => Some(InferredType::primitive("Int")),
        ExprKind::Bool(_) => Some(InferredType::primitive("Bool")),
        ExprKind::String { .. } => Some(InferredType::primitive("String")),
        ExprKind::Tuple(items) => {
            let items = items
                .iter()
                .map(|item| infer_dependency_generic_expr_type(item, bindings, function_bindings))
                .collect::<Option<Vec<_>>>()?;
            Some(InferredType {
                rendered: render_inferred_tuple_type(&items),
                kind: InferredTypeKind::Tuple(items),
            })
        }
        ExprKind::Array(items) => {
            let (first, rest) = items.split_first()?;
            let element = infer_dependency_generic_expr_type(first, bindings, function_bindings)?;
            for item in rest {
                let item_ty =
                    infer_dependency_generic_expr_type(item, bindings, function_bindings)?;
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
        ExprKind::RepeatArray { value, len, .. } => {
            let element = infer_dependency_generic_expr_type(value, bindings, function_bindings)?;
            Some(InferredType {
                rendered: format!("[{}; {len}]", element.rendered),
                kind: InferredTypeKind::Array {
                    element: Box::new(element),
                    len: len.clone(),
                },
            })
        }
        ExprKind::Name(name) => bindings.get(name).cloned(),
        ExprKind::Block(block) | ExprKind::Unsafe(block) => {
            infer_dependency_generic_block_type(block, bindings, function_bindings)
        }
        ExprKind::If {
            then_branch,
            else_branch,
            ..
        } => {
            let then_ty =
                infer_dependency_generic_block_type(then_branch, bindings, function_bindings)?;
            let else_ty = infer_dependency_generic_expr_type(
                else_branch.as_deref()?,
                bindings,
                function_bindings,
            )?;
            (then_ty == else_ty).then_some(then_ty)
        }
        ExprKind::Match { arms, .. } => {
            let (first, rest) = arms.split_first()?;
            let first_ty =
                infer_dependency_generic_expr_type(&first.body, bindings, function_bindings)?;
            for arm in rest {
                let arm_ty =
                    infer_dependency_generic_expr_type(&arm.body, bindings, function_bindings)?;
                if arm_ty != first_ty {
                    return None;
                }
            }
            Some(first_ty)
        }
        ExprKind::Call { callee, args } => {
            infer_single_field_generic_variant_call_type(callee, args, bindings, function_bindings)
                .or_else(|| {
                    infer_function_call_return_type(callee, args, bindings, function_bindings)
                })
        }
        ExprKind::Bracket { target, items } => {
            infer_dependency_generic_projection_type(target, items, bindings, function_bindings)
        }
        ExprKind::Binary { left, op, right } => {
            let left = infer_dependency_generic_expr_type(left, bindings, function_bindings)?;
            let right = infer_dependency_generic_expr_type(right, bindings, function_bindings)?;
            infer_dependency_generic_binary_expr_type(*op, &left, &right)
        }
        ExprKind::Unary { op, expr } => {
            let expr = infer_dependency_generic_expr_type(expr, bindings, function_bindings)?;
            infer_dependency_generic_unary_expr_type(*op, &expr)
        }
        _ => None,
    }
}

fn infer_dependency_generic_block_type(
    block: &ql_ast::Block,
    bindings: &ValueTypeBindings,
    function_bindings: &FunctionTypeBindings,
) -> Option<InferredType> {
    let mut block_bindings = bindings.clone();
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
                function_bindings,
            );
        }
    }
    infer_dependency_generic_expr_type(block.tail.as_deref()?, &block_bindings, function_bindings)
}

fn infer_dependency_generic_projection_type(
    target: &Expr,
    items: &[Expr],
    bindings: &ValueTypeBindings,
    function_bindings: &FunctionTypeBindings,
) -> Option<InferredType> {
    let [index] = items else {
        return None;
    };
    let target_ty = infer_dependency_generic_expr_type(target, bindings, function_bindings)?;
    match target_ty.kind {
        InferredTypeKind::Array { element, .. } => {
            let index_ty = infer_dependency_generic_expr_type(index, bindings, function_bindings)?;
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

fn infer_dependency_generic_binary_expr_type(
    op: BinaryOp,
    left: &InferredType,
    right: &InferredType,
) -> Option<InferredType> {
    match op {
        BinaryOp::OrOr | BinaryOp::AndAnd if are_inferred_bool_types(left, right) => {
            Some(InferredType::primitive("Bool"))
        }
        BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem
            if left == right && is_inferred_numeric_type(left) =>
        {
            Some(left.clone())
        }
        BinaryOp::EqEq | BinaryOp::BangEq
            if left == right && is_inferred_equality_comparable_type(left) =>
        {
            Some(InferredType::primitive("Bool"))
        }
        BinaryOp::Gt | BinaryOp::GtEq | BinaryOp::Lt | BinaryOp::LtEq
            if left == right && is_inferred_ordered_comparable_type(left) =>
        {
            Some(InferredType::primitive("Bool"))
        }
        BinaryOp::Assign => None,
        _ => None,
    }
}

fn infer_dependency_generic_unary_expr_type(
    op: UnaryOp,
    expr: &InferredType,
) -> Option<InferredType> {
    match op {
        UnaryOp::Not if is_inferred_bool_type(expr) => Some(InferredType::primitive("Bool")),
        UnaryOp::Neg if is_inferred_numeric_type(expr) => Some(expr.clone()),
        UnaryOp::Await | UnaryOp::Spawn => None,
        _ => None,
    }
}

fn infer_single_field_generic_variant_call_type(
    callee: &Expr,
    args: &[CallArg],
    bindings: &ValueTypeBindings,
    function_bindings: &FunctionTypeBindings,
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
    let arg_ty = infer_dependency_generic_expr_type(value, bindings, function_bindings)?;
    Some(InferredType {
        rendered: format!("{type_name}[{}]", arg_ty.rendered),
        kind: InferredTypeKind::Named {
            path: vec![type_name.clone()],
            args: vec![arg_ty],
        },
    })
}

#[cfg(test)]
#[path = "expr_inference_tests.rs"]
mod tests;
