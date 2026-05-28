use std::collections::BTreeMap;

use ql_ast::{Expr, FunctionDecl, ItemKind, Module, Param, Pattern, PatternKind, TypeExpr};

use super::enum_bindings::{EnumTypeBindings, tuple_variant_field_types};
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
    enum_bindings: &EnumTypeBindings,
) {
    record_let_type_bindings_with_substitutions(
        pattern,
        ty,
        value,
        bindings,
        function_bindings,
        enum_bindings,
        None,
    );
}

pub(super) fn record_let_type_bindings_with_substitutions(
    pattern: &Pattern,
    ty: Option<&TypeExpr>,
    value: &Expr,
    bindings: &mut ValueTypeBindings,
    function_bindings: &FunctionTypeBindings,
    enum_bindings: &EnumTypeBindings,
    substitutions: Option<&TypeSubstitutions>,
) {
    if let Some(ty) = ty {
        record_pattern_type_bindings_with_substitutions(
            pattern,
            ty,
            bindings,
            enum_bindings,
            substitutions,
        );
        return;
    }
    if let Some(ty) =
        infer_dependency_generic_expr_type(value, bindings, function_bindings, enum_bindings)
    {
        record_pattern_inferred_type_bindings(pattern, &ty, bindings, enum_bindings);
    }
}

pub(super) fn record_iterable_type_bindings(
    pattern: &Pattern,
    iterable: &Expr,
    bindings: &mut ValueTypeBindings,
    function_bindings: &FunctionTypeBindings,
    enum_bindings: &EnumTypeBindings,
) {
    let Some(iterable_ty) =
        infer_dependency_generic_expr_type(iterable, bindings, function_bindings, enum_bindings)
    else {
        return;
    };
    let InferredTypeKind::Array { element, .. } = &iterable_ty.kind else {
        return;
    };
    record_pattern_inferred_type_bindings(pattern, element, bindings, enum_bindings);
}

fn record_pattern_type_bindings_with_substitutions(
    pattern: &Pattern,
    ty: &TypeExpr,
    bindings: &mut ValueTypeBindings,
    enum_bindings: &EnumTypeBindings,
    substitutions: Option<&TypeSubstitutions>,
) {
    let Some(substitutions) = substitutions else {
        record_pattern_type_bindings(pattern, ty, bindings, enum_bindings);
        return;
    };
    if let Some(ty) = inferred_type_from_type_expr_with_substitutions(ty, substitutions) {
        record_pattern_inferred_type_bindings(pattern, &ty, bindings, enum_bindings);
    }
}

fn record_pattern_type_bindings(
    pattern: &Pattern,
    ty: &TypeExpr,
    bindings: &mut ValueTypeBindings,
    enum_bindings: &EnumTypeBindings,
) {
    if let Some(ty) = InferredType::from_type_expr(ty) {
        record_pattern_inferred_type_bindings(pattern, &ty, bindings, enum_bindings);
    }
}

pub(super) fn record_pattern_inferred_type_bindings(
    pattern: &Pattern,
    ty: &InferredType,
    bindings: &mut ValueTypeBindings,
    enum_bindings: &EnumTypeBindings,
) {
    match (&pattern.kind, &ty.kind) {
        (PatternKind::Name(name), _) => {
            bindings.insert(name.clone(), ty.clone());
        }
        (PatternKind::Tuple(patterns), super::inferred_types::InferredTypeKind::Tuple(types))
            if patterns.len() == types.len() =>
        {
            for (pattern, ty) in patterns.iter().zip(types) {
                record_pattern_inferred_type_bindings(pattern, ty, bindings, enum_bindings);
            }
        }
        (PatternKind::Array(patterns), InferredTypeKind::Array { element, .. }) => {
            for pattern in patterns {
                record_pattern_inferred_type_bindings(pattern, element, bindings, enum_bindings);
            }
        }
        (
            PatternKind::TupleStruct { path, items },
            InferredTypeKind::Named {
                path: enum_path,
                args,
            },
        ) => {
            if let Some(field_types) =
                tuple_struct_field_types(path, enum_path, args, enum_bindings)
                && items.len() == field_types.len()
            {
                for (pattern, ty) in items.iter().zip(field_types) {
                    record_pattern_inferred_type_bindings(pattern, &ty, bindings, enum_bindings);
                }
            }
        }
        _ => {}
    }
}

fn tuple_struct_field_types(
    pattern_path: &ql_ast::Path,
    enum_path: &[String],
    enum_args: &[InferredType],
    enum_bindings: &EnumTypeBindings,
) -> Option<Vec<InferredType>> {
    let enum_name = enum_path.last()?;
    let pattern_enum_name = pattern_path.segments.first()?;
    if enum_name != pattern_enum_name {
        return None;
    }
    let variant_name = pattern_path.segments.last()?;
    tuple_variant_field_types(enum_name, variant_name, enum_args, enum_bindings)
}
