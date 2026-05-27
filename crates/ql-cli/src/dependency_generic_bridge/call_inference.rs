use std::collections::BTreeSet;

use ql_ast::{CallArg, Expr, ExprKind, FunctionDecl, Param, TypeExpr, TypeExprKind};

use super::expr_inference::{ValueTypeBindings, infer_dependency_generic_expr_type};
use super::function_bindings::FunctionTypeBindings;
use super::inferred_types::{
    InferredType, inferred_type_from_type_expr_with_substitutions, type_expr_from_inferred_type,
};
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

pub(super) fn call_arg_expr(arg: &CallArg) -> &Expr {
    match arg {
        CallArg::Positional(expr) | CallArg::Named { value: expr, .. } => expr,
    }
}

fn ordered_dependency_generic_call_args<'f, 'a>(
    function: &'f FunctionDecl,
    args: &'a [CallArg],
) -> Option<Vec<(&'f TypeExpr, &'a CallArg)>> {
    let regular_params = function
        .params
        .iter()
        .filter_map(|param| match param {
            Param::Regular { name, ty, .. } => Some((name.as_str(), ty)),
            Param::Receiver { .. } => None,
        })
        .collect::<Vec<_>>();
    if args.iter().all(|arg| matches!(arg, CallArg::Positional(_))) {
        return (regular_params.len() == args.len()).then(|| {
            regular_params
                .into_iter()
                .zip(args)
                .map(|((_, param_ty), arg)| (param_ty, arg))
                .collect()
        });
    }

    let mut ordered = vec![None; regular_params.len()];
    let mut next_positional = 0usize;
    let mut named_started = false;
    for arg in args {
        let index = match arg {
            CallArg::Named { name, .. } => {
                named_started = true;
                regular_params
                    .iter()
                    .position(|(param_name, _)| *param_name == name.as_str())?
            }
            CallArg::Positional(_) => {
                if named_started {
                    return None;
                }
                while next_positional < ordered.len() && ordered[next_positional].is_some() {
                    next_positional += 1;
                }
                if next_positional == ordered.len() {
                    return None;
                }
                next_positional
            }
        };
        if ordered[index].is_some() {
            return None;
        }
        ordered[index] = Some(arg);
    }

    regular_params
        .into_iter()
        .zip(ordered)
        .map(|((_, param_ty), arg)| arg.map(|arg| (param_ty, arg)))
        .collect()
}

pub(super) fn ordered_call_arg_expected_types(
    callee: &Expr,
    args: &[CallArg],
    expected_ty: Option<&TypeExpr>,
    bindings: &ValueTypeBindings,
    function_bindings: &FunctionTypeBindings,
) -> Vec<Option<TypeExpr>> {
    let mut expected_types = vec![None; args.len()];
    let ExprKind::Name(name) = &callee.kind else {
        return expected_types;
    };
    let Some(function) = function_bindings.get(name) else {
        return expected_types;
    };
    let generic_names = function
        .generics
        .iter()
        .map(|generic| generic.name.as_str())
        .collect::<BTreeSet<_>>();
    let mut substitutions = expected_ty
        .and_then(|expected_ty| {
            infer_dependency_generic_return_substitutions(function, expected_ty)
        })
        .unwrap_or_default();
    let Some(ordered_args) = ordered_dependency_generic_call_args(function, args) else {
        return expected_types;
    };

    for (param_ty, arg) in ordered_args {
        if !type_expr_mentions_generic(param_ty, &generic_names) {
            continue;
        }
        let _ = collect_generic_type_substitutions_from_arg_expr(
            param_ty,
            arg,
            &generic_names,
            bindings,
            function_bindings,
            &mut substitutions,
        );
    }

    let Some(ordered_args) = ordered_dependency_generic_call_args(function, args) else {
        return expected_types;
    };
    for (param_ty, arg) in ordered_args {
        let Some(source_index) = args
            .iter()
            .position(|candidate| std::ptr::eq(candidate, arg))
        else {
            continue;
        };
        expected_types[source_index] =
            inferred_type_from_type_expr_with_substitutions(param_ty, &substitutions)
                .map(|ty| type_expr_from_inferred_type(&ty))
                .or_else(|| Some(param_ty.clone()));
    }

    expected_types
}

fn infer_dependency_generic_return_substitutions(
    function: &FunctionDecl,
    expected_ty: &TypeExpr,
) -> Option<TypeSubstitutions> {
    let return_ty = function.return_type.as_ref()?;
    let generic_names = function
        .generics
        .iter()
        .map(|generic| generic.name.as_str())
        .collect::<BTreeSet<_>>();
    if !type_expr_mentions_generic(return_ty, &generic_names) {
        return Some(TypeSubstitutions::new());
    }
    let expected_ty = InferredType::from_type_expr(expected_ty)?;
    let mut substitutions = TypeSubstitutions::new();
    collect_generic_type_substitutions(return_ty, &expected_ty, &generic_names, &mut substitutions)
        .then_some(substitutions)
}

fn collect_generic_type_substitutions_from_arg_expr(
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
