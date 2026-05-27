use std::collections::{BTreeMap, BTreeSet};

use ql_ast::{TypeExpr, TypeExprKind};

use super::inferred_types::{InferredType, InferredTypeKind};

pub(super) type TypeSubstitutions = BTreeMap<String, String>;

pub(super) fn generic_param_name_for_type_expr<'a>(
    ty: &TypeExpr,
    generic_names: &BTreeSet<&'a str>,
) -> Option<&'a str> {
    let TypeExprKind::Named { path, args } = &ty.kind else {
        return None;
    };
    if !args.is_empty() {
        return None;
    }
    let [name] = path.segments.as_slice() else {
        return None;
    };
    generic_names.get(name.as_str()).copied()
}

pub(super) fn type_expr_mentions_generic(ty: &TypeExpr, generic_names: &BTreeSet<&str>) -> bool {
    if generic_param_name_for_type_expr(ty, generic_names).is_some() {
        return true;
    }
    match &ty.kind {
        TypeExprKind::Pointer { inner, .. } => type_expr_mentions_generic(inner, generic_names),
        TypeExprKind::Array { element, len } => {
            generic_names.contains(len.as_str())
                || type_expr_mentions_generic(element, generic_names)
        }
        TypeExprKind::Named { args, .. } | TypeExprKind::Tuple(args) => args
            .iter()
            .any(|arg| type_expr_mentions_generic(arg, generic_names)),
        TypeExprKind::Callable { params, ret } => {
            params
                .iter()
                .any(|param| type_expr_mentions_generic(param, generic_names))
                || type_expr_mentions_generic(ret, generic_names)
        }
    }
}

pub(super) fn collect_generic_type_substitutions(
    param_ty: &TypeExpr,
    arg_ty: &InferredType,
    generic_names: &BTreeSet<&str>,
    substitutions: &mut TypeSubstitutions,
) -> bool {
    if let Some(generic_name) = generic_param_name_for_type_expr(param_ty, generic_names) {
        return bind_generic_type_substitution(generic_name, arg_ty, substitutions);
    }

    match (&param_ty.kind, &arg_ty.kind) {
        (
            TypeExprKind::Named { path, args },
            InferredTypeKind::Named {
                path: arg_path,
                args: arg_args,
            },
        ) => collect_named_type_substitutions(
            &path.segments,
            args,
            arg_path,
            arg_args,
            generic_names,
            substitutions,
        ),
        (TypeExprKind::Tuple(params), InferredTypeKind::Tuple(args)) => {
            collect_type_arg_substitutions(params, args, generic_names, substitutions)
        }
        (
            TypeExprKind::Array {
                element: param_element,
                len: param_len,
            },
            InferredTypeKind::Array {
                element: arg_element,
                len: arg_len,
            },
        ) => collect_array_type_substitutions(
            param_element,
            param_len,
            arg_element,
            arg_len,
            generic_names,
            substitutions,
        ),
        (
            TypeExprKind::Pointer {
                is_const: param_const,
                inner: param_inner,
            },
            InferredTypeKind::Pointer {
                is_const: arg_const,
                inner: arg_inner,
            },
        ) => collect_pointer_type_substitutions(
            *param_const,
            param_inner,
            *arg_const,
            arg_inner,
            generic_names,
            substitutions,
        ),
        (
            TypeExprKind::Callable {
                params: param_params,
                ret: param_ret,
            },
            InferredTypeKind::Callable {
                params: arg_params,
                ret: arg_ret,
            },
        ) => collect_callable_type_substitutions(
            param_params,
            param_ret,
            arg_params,
            arg_ret,
            generic_names,
            substitutions,
        ),
        _ => false,
    }
}

fn collect_named_type_substitutions(
    param_path: &[String],
    param_args: &[TypeExpr],
    arg_path: &[String],
    arg_args: &[InferredType],
    generic_names: &BTreeSet<&str>,
    substitutions: &mut TypeSubstitutions,
) -> bool {
    param_path == arg_path
        && collect_type_arg_substitutions(param_args, arg_args, generic_names, substitutions)
}

fn collect_array_type_substitutions(
    param_element: &TypeExpr,
    param_len: &str,
    arg_element: &InferredType,
    arg_len: &str,
    generic_names: &BTreeSet<&str>,
    substitutions: &mut TypeSubstitutions,
) -> bool {
    bind_generic_len_substitution(param_len, arg_len, generic_names, substitutions)
        && collect_generic_type_substitutions(
            param_element,
            arg_element,
            generic_names,
            substitutions,
        )
}

fn collect_pointer_type_substitutions(
    param_const: bool,
    param_inner: &TypeExpr,
    arg_const: bool,
    arg_inner: &InferredType,
    generic_names: &BTreeSet<&str>,
    substitutions: &mut TypeSubstitutions,
) -> bool {
    param_const == arg_const
        && collect_generic_type_substitutions(param_inner, arg_inner, generic_names, substitutions)
}

fn collect_callable_type_substitutions(
    param_params: &[TypeExpr],
    param_ret: &TypeExpr,
    arg_params: &[InferredType],
    arg_ret: &InferredType,
    generic_names: &BTreeSet<&str>,
    substitutions: &mut TypeSubstitutions,
) -> bool {
    collect_type_arg_substitutions(param_params, arg_params, generic_names, substitutions)
        && collect_generic_type_substitutions(param_ret, arg_ret, generic_names, substitutions)
}

fn collect_type_arg_substitutions(
    param_args: &[TypeExpr],
    arg_args: &[InferredType],
    generic_names: &BTreeSet<&str>,
    substitutions: &mut TypeSubstitutions,
) -> bool {
    param_args.len() == arg_args.len()
        && param_args.iter().zip(arg_args).all(|(param_arg, arg_arg)| {
            collect_generic_type_substitutions(param_arg, arg_arg, generic_names, substitutions)
        })
}

pub(super) fn bind_generic_len_substitution(
    param_len: &str,
    arg_len: &str,
    generic_names: &BTreeSet<&str>,
    substitutions: &mut TypeSubstitutions,
) -> bool {
    if generic_names.contains(param_len) {
        return bind_generic_rendered_substitution(param_len, arg_len, substitutions);
    }
    param_len == arg_len
}

pub(super) fn bind_generic_type_substitution(
    generic_name: &str,
    arg_ty: &InferredType,
    substitutions: &mut TypeSubstitutions,
) -> bool {
    bind_generic_rendered_substitution(generic_name, &arg_ty.rendered, substitutions)
}

fn bind_generic_rendered_substitution(
    generic_name: &str,
    rendered: &str,
    substitutions: &mut TypeSubstitutions,
) -> bool {
    match substitutions.get(generic_name) {
        Some(existing) => existing == rendered,
        None => {
            substitutions.insert(generic_name.to_owned(), rendered.to_owned());
            true
        }
    }
}

#[cfg(test)]
#[path = "substitutions_tests.rs"]
mod tests;
