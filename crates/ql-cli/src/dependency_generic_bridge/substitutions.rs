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
        ) => {
            path.segments == *arg_path
                && args.len() == arg_args.len()
                && args.iter().zip(arg_args).all(|(param_arg, arg_arg)| {
                    collect_generic_type_substitutions(
                        param_arg,
                        arg_arg,
                        generic_names,
                        substitutions,
                    )
                })
        }
        (TypeExprKind::Tuple(params), InferredTypeKind::Tuple(args)) => {
            params.len() == args.len()
                && params.iter().zip(args).all(|(param_item, arg_item)| {
                    collect_generic_type_substitutions(
                        param_item,
                        arg_item,
                        generic_names,
                        substitutions,
                    )
                })
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
        ) => {
            bind_generic_len_substitution(param_len, arg_len, generic_names, substitutions)
                && collect_generic_type_substitutions(
                    param_element,
                    arg_element,
                    generic_names,
                    substitutions,
                )
        }
        (
            TypeExprKind::Pointer {
                is_const: param_const,
                inner: param_inner,
            },
            InferredTypeKind::Pointer {
                is_const: arg_const,
                inner: arg_inner,
            },
        ) => {
            param_const == arg_const
                && collect_generic_type_substitutions(
                    param_inner,
                    arg_inner,
                    generic_names,
                    substitutions,
                )
        }
        (
            TypeExprKind::Callable {
                params: param_params,
                ret: param_ret,
            },
            InferredTypeKind::Callable {
                params: arg_params,
                ret: arg_ret,
            },
        ) => {
            param_params.len() == arg_params.len()
                && param_params
                    .iter()
                    .zip(arg_params)
                    .all(|(param_param, arg_param)| {
                        collect_generic_type_substitutions(
                            param_param,
                            arg_param,
                            generic_names,
                            substitutions,
                        )
                    })
                && collect_generic_type_substitutions(
                    param_ret,
                    arg_ret,
                    generic_names,
                    substitutions,
                )
        }
        _ => false,
    }
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
mod tests {
    use ql_ast::{ItemKind, Param};

    use super::*;

    fn parse_type_expr(rendered: &str) -> TypeExpr {
        let source = format!("fn __probe(value: {rendered}) -> Int {{ return 0 }}");
        let module = ql_parser::parse_source(&source).expect("type probe should parse");
        module
            .items
            .into_iter()
            .find_map(|item| {
                let ItemKind::Function(function) = item.kind else {
                    return None;
                };
                function.params.into_iter().find_map(|param| match param {
                    Param::Regular { ty, .. } => Some(ty),
                    Param::Receiver { .. } => None,
                })
            })
            .expect("probe function should contain a typed value parameter")
    }

    #[test]
    fn collects_nested_type_and_array_length_substitutions() {
        let param_ty = parse_type_expr("Result[T, [U; N]]");
        let arg_ty = InferredType::from_type_expr(&parse_type_expr("Result[Int, [String; 3]]"))
            .expect("argument type should infer");
        let generic_names = BTreeSet::from(["T", "U", "N"]);
        let mut substitutions = TypeSubstitutions::new();

        assert!(collect_generic_type_substitutions(
            &param_ty,
            &arg_ty,
            &generic_names,
            &mut substitutions
        ));
        assert_eq!(substitutions.get("T").map(String::as_str), Some("Int"));
        assert_eq!(substitutions.get("U").map(String::as_str), Some("String"));
        assert_eq!(substitutions.get("N").map(String::as_str), Some("3"));
    }

    #[test]
    fn rejects_conflicting_type_substitutions() {
        let param_ty = parse_type_expr("(T, T)");
        let arg_ty = InferredType::from_type_expr(&parse_type_expr("(Int, Bool)"))
            .expect("argument type should infer");
        let generic_names = BTreeSet::from(["T"]);
        let mut substitutions = TypeSubstitutions::new();

        assert!(!collect_generic_type_substitutions(
            &param_ty,
            &arg_ty,
            &generic_names,
            &mut substitutions
        ));
    }
}
