use std::collections::BTreeMap;

use ql_ast::{
    Expr, FunctionDecl, ItemKind, Module, Param, Pattern, PatternKind, TypeExpr, TypeExprKind,
};

use super::expr_inference::infer_dependency_generic_expr_type;
use super::function_bindings::FunctionTypeBindings;
use super::inferred_type_conversion::inferred_type_from_type_expr_with_substitutions;
use super::inferred_types::{InferredType, InferredTypeKind};
use super::substitutions::TypeSubstitutions;

pub(super) type ValueTypeBindings = BTreeMap<String, InferredType>;

pub(super) fn collect_root_value_type_bindings(root_module: &Module) -> ValueTypeBindings {
    let mut bindings = ValueTypeBindings::new();
    for item in &root_module.items {
        let (ItemKind::Const(global) | ItemKind::Static(global)) = &item.kind else {
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
    record_let_type_bindings_with_substitutions(
        pattern,
        ty,
        value,
        bindings,
        function_bindings,
        None,
    );
}

pub(super) fn record_let_type_bindings_with_substitutions(
    pattern: &Pattern,
    ty: Option<&TypeExpr>,
    value: &Expr,
    bindings: &mut ValueTypeBindings,
    function_bindings: &FunctionTypeBindings,
    substitutions: Option<&TypeSubstitutions>,
) {
    if let Some(ty) = ty {
        record_pattern_type_bindings_with_substitutions(pattern, ty, bindings, substitutions);
        return;
    }
    if let Some(ty) = infer_dependency_generic_expr_type(value, bindings, function_bindings) {
        record_pattern_inferred_type_bindings(pattern, &ty, bindings);
    }
}

pub(super) fn record_iterable_type_bindings(
    pattern: &Pattern,
    iterable: &Expr,
    bindings: &mut ValueTypeBindings,
    function_bindings: &FunctionTypeBindings,
) {
    let Some(iterable_ty) =
        infer_dependency_generic_expr_type(iterable, bindings, function_bindings)
    else {
        return;
    };
    let InferredTypeKind::Array { element, .. } = &iterable_ty.kind else {
        return;
    };
    record_pattern_inferred_type_bindings(pattern, element, bindings);
}

fn record_pattern_type_bindings_with_substitutions(
    pattern: &Pattern,
    ty: &TypeExpr,
    bindings: &mut ValueTypeBindings,
    substitutions: Option<&TypeSubstitutions>,
) {
    let Some(substitutions) = substitutions else {
        record_pattern_type_bindings(pattern, ty, bindings);
        return;
    };
    if let Some(ty) = inferred_type_from_type_expr_with_substitutions(ty, substitutions) {
        record_pattern_inferred_type_bindings(pattern, &ty, bindings);
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
        (PatternKind::Array(patterns), TypeExprKind::Array { element, .. }) => {
            for pattern in patterns {
                record_pattern_type_bindings(pattern, element, bindings);
            }
        }
        _ => {}
    }
}

fn record_pattern_inferred_type_bindings(
    pattern: &Pattern,
    ty: &InferredType,
    bindings: &mut ValueTypeBindings,
) {
    match (&pattern.kind, &ty.kind) {
        (PatternKind::Name(name), _) => {
            bindings.insert(name.clone(), ty.clone());
        }
        (PatternKind::Tuple(patterns), super::inferred_types::InferredTypeKind::Tuple(types))
            if patterns.len() == types.len() =>
        {
            for (pattern, ty) in patterns.iter().zip(types) {
                record_pattern_inferred_type_bindings(pattern, ty, bindings);
            }
        }
        (PatternKind::Array(patterns), InferredTypeKind::Array { element, .. }) => {
            for pattern in patterns {
                record_pattern_inferred_type_bindings(pattern, element, bindings);
            }
        }
        _ => {}
    }
}
