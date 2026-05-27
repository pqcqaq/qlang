use std::collections::BTreeSet;

use ql_ast::{CallArg, Expr, ExprKind, FunctionDecl, TypeExpr, TypeExprKind};

use super::call_args::{call_arg_expr, ordered_dependency_generic_call_args};
use super::expr_inference::{ValueTypeBindings, infer_dependency_generic_expr_type};
use super::function_bindings::FunctionTypeBindings;
use super::inferred_types::{InferredType, inferred_type_from_type_expr_with_substitutions};
use super::substitutions::{
    TypeSubstitutions, bind_generic_len_substitution, bind_generic_type_substitution,
    collect_generic_type_substitutions, generic_param_name_for_type_expr,
    type_expr_mentions_generic,
};

pub(super) fn infer_dependency_generic_function_substitutions(
    function: &FunctionDecl,
    args: &[CallArg],
    expected_ty: Option<&TypeExpr>,
    bindings: &ValueTypeBindings,
    function_bindings: &FunctionTypeBindings,
) -> Option<TypeSubstitutions> {
    let ordered_args = ordered_dependency_generic_call_args(function, args)?;
    let generic_names = function
        .generics
        .iter()
        .map(|generic| generic.name.as_str())
        .collect::<BTreeSet<_>>();
    let mut substitutions = TypeSubstitutions::new();
    for (param_ty, arg) in ordered_args {
        if !type_expr_mentions_generic(param_ty, &generic_names) {
            continue;
        }
        if !collect_generic_type_substitutions_from_arg_expr(
            param_ty,
            arg,
            &generic_names,
            bindings,
            function_bindings,
            &mut substitutions,
        ) {
            return None;
        }
    }
    if let (Some(return_ty), Some(expected_ty)) = (function.return_type.as_ref(), expected_ty)
        && type_expr_mentions_generic(return_ty, &generic_names)
    {
        let expected_ty = InferredType::from_type_expr(expected_ty)?;
        if !collect_generic_type_substitutions(
            return_ty,
            &expected_ty,
            &generic_names,
            &mut substitutions,
        ) {
            return None;
        }
    }
    Some(substitutions)
}

pub(super) fn collect_generic_type_substitutions_from_arg_expr(
    param_ty: &TypeExpr,
    arg: &CallArg,
    generic_names: &BTreeSet<&str>,
    bindings: &ValueTypeBindings,
    function_bindings: &FunctionTypeBindings,
    substitutions: &mut TypeSubstitutions,
) -> bool {
    collect_generic_type_substitutions_from_expr(
        param_ty,
        call_arg_expr(arg),
        generic_names,
        bindings,
        function_bindings,
        substitutions,
    )
}

fn collect_generic_type_substitutions_from_expr(
    param_ty: &TypeExpr,
    expr: &Expr,
    generic_names: &BTreeSet<&str>,
    bindings: &ValueTypeBindings,
    function_bindings: &FunctionTypeBindings,
    substitutions: &mut TypeSubstitutions,
) -> bool {
    if let Some(generic_name) = generic_param_name_for_type_expr(param_ty, generic_names) {
        return infer_dependency_generic_expr_type(expr, bindings, function_bindings).is_none_or(
            |arg_ty| bind_generic_type_substitution(generic_name, &arg_ty, substitutions),
        );
    }

    match (&param_ty.kind, &expr.kind) {
        (
            TypeExprKind::Array {
                element: param_element,
                len: param_len,
            },
            ExprKind::Array(items),
        ) => {
            bind_generic_len_substitution(
                param_len,
                &items.len().to_string(),
                generic_names,
                substitutions,
            ) && items.iter().all(|item| {
                collect_generic_type_substitutions_from_expr(
                    param_element,
                    item,
                    generic_names,
                    bindings,
                    function_bindings,
                    substitutions,
                )
            })
        }
        (
            TypeExprKind::Array {
                element: param_element,
                len: param_len,
            },
            ExprKind::RepeatArray { value, len, .. },
        ) => {
            bind_generic_len_substitution(param_len, len, generic_names, substitutions)
                && collect_generic_type_substitutions_from_expr(
                    param_element,
                    value,
                    generic_names,
                    bindings,
                    function_bindings,
                    substitutions,
                )
        }
        (TypeExprKind::Tuple(param_items), ExprKind::Tuple(items))
            if param_items.len() == items.len() =>
        {
            param_items.iter().zip(items).all(|(param_item, item)| {
                collect_generic_type_substitutions_from_expr(
                    param_item,
                    item,
                    generic_names,
                    bindings,
                    function_bindings,
                    substitutions,
                )
            })
        }
        _ => infer_dependency_generic_expr_type(expr, bindings, function_bindings).is_none_or(
            |arg_ty| {
                collect_generic_type_substitutions(param_ty, &arg_ty, generic_names, substitutions)
            },
        ),
    }
}

pub(super) fn infer_function_call_return_type(
    callee: &Expr,
    args: &[CallArg],
    bindings: &ValueTypeBindings,
    function_bindings: &FunctionTypeBindings,
) -> Option<InferredType> {
    let ExprKind::Name(name) = &callee.kind else {
        return None;
    };
    let function = function_bindings.get(name)?;
    let return_ty = function.return_type.as_ref()?;
    let substitutions = infer_dependency_generic_function_substitutions(
        function,
        args,
        None,
        bindings,
        function_bindings,
    )?;
    inferred_type_from_type_expr_with_substitutions(return_ty, &substitutions)
}

#[cfg(test)]
#[path = "call_inference_tests.rs"]
mod tests;
