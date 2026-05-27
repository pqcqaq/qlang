use ql_ast::{CallArg, Expr, ExprKind, FunctionDecl, Param, TypeExpr};

use super::call_inference::collect_generic_type_substitutions_from_arg_expr;
use super::expr_inference::ValueTypeBindings;
use super::function_bindings::FunctionTypeBindings;
use super::inferred_type_conversion::{
    inferred_type_from_type_expr_with_substitutions, type_expr_from_inferred_type,
};
use super::inferred_types::InferredType;
use super::substitutions::{
    TypeSubstitutions, collect_generic_type_substitutions, type_expr_mentions_generic,
};

pub(super) fn call_arg_expr(arg: &CallArg) -> &Expr {
    match arg {
        CallArg::Positional(expr) | CallArg::Named { value: expr, .. } => expr,
    }
}

pub(super) fn ordered_dependency_generic_call_args<'f, 'a>(
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
        .collect::<std::collections::BTreeSet<_>>();
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
        .collect::<std::collections::BTreeSet<_>>();
    if !type_expr_mentions_generic(return_ty, &generic_names) {
        return Some(TypeSubstitutions::new());
    }
    let expected_ty = InferredType::from_type_expr(expected_ty)?;
    let mut substitutions = TypeSubstitutions::new();
    collect_generic_type_substitutions(return_ty, &expected_ty, &generic_names, &mut substitutions)
        .then_some(substitutions)
}
