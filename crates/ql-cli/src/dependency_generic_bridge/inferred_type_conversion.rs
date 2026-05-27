use std::collections::BTreeMap;

use ql_ast::{ItemKind, Param, Path, TypeExpr, TypeExprKind};
use ql_span::Span;

use super::inferred_types::{InferredType, InferredTypeKind, render_inferred_tuple_type};

pub(super) fn inferred_type_from_type_expr_with_substitutions(
    ty: &TypeExpr,
    substitutions: &BTreeMap<String, String>,
) -> Option<InferredType> {
    match &ty.kind {
        TypeExprKind::Named { path, args } => {
            if args.is_empty()
                && let [name] = path.segments.as_slice()
                && let Some(substitution) = substitutions.get(name)
            {
                return Some(inferred_type_from_rendered_substitution(substitution));
            }

            let args = args
                .iter()
                .map(|arg| inferred_type_from_type_expr_with_substitutions(arg, substitutions))
                .collect::<Option<Vec<_>>>()?;
            Some(InferredType::named(path.segments.clone(), args))
        }
        TypeExprKind::Tuple(items) => {
            let items = items
                .iter()
                .map(|item| inferred_type_from_type_expr_with_substitutions(item, substitutions))
                .collect::<Option<Vec<_>>>()?;
            Some(InferredType {
                rendered: render_inferred_tuple_type(&items),
                kind: InferredTypeKind::Tuple(items),
            })
        }
        TypeExprKind::Array { element, len } => {
            let element = inferred_type_from_type_expr_with_substitutions(element, substitutions)?;
            let len = substitutions
                .get(len)
                .cloned()
                .unwrap_or_else(|| len.clone());
            Some(InferredType::array(element, len))
        }
        TypeExprKind::Pointer { is_const, inner } => {
            let inner = inferred_type_from_type_expr_with_substitutions(inner, substitutions)?;
            let qualifier = if *is_const { "const " } else { "" };
            Some(InferredType {
                rendered: format!("*{}{}", qualifier, inner.rendered),
                kind: InferredTypeKind::Pointer {
                    is_const: *is_const,
                    inner: Box::new(inner),
                },
            })
        }
        TypeExprKind::Callable { params, ret } => {
            let params = params
                .iter()
                .map(|param| inferred_type_from_type_expr_with_substitutions(param, substitutions))
                .collect::<Option<Vec<_>>>()?;
            let ret = inferred_type_from_type_expr_with_substitutions(ret, substitutions)?;
            Some(InferredType::callable(params, ret))
        }
    }
}

pub(super) fn inferred_type_from_rendered_substitution(rendered: &str) -> InferredType {
    if let Some(ty) = parse_rendered_type_substitution(rendered)
        && let Some(ty) = InferredType::from_type_expr(&ty)
    {
        return ty;
    }
    InferredType::named(rendered.split('.').map(str::to_owned).collect(), Vec::new())
}

pub(super) fn type_expr_from_inferred_type(ty: &InferredType) -> TypeExpr {
    match &ty.kind {
        InferredTypeKind::Pointer { is_const, inner } => TypeExpr::new(
            Span::default(),
            TypeExprKind::Pointer {
                is_const: *is_const,
                inner: Box::new(type_expr_from_inferred_type(inner)),
            },
        ),
        InferredTypeKind::Array { element, len } => TypeExpr::new(
            Span::default(),
            TypeExprKind::Array {
                element: Box::new(type_expr_from_inferred_type(element)),
                len: len.clone(),
            },
        ),
        InferredTypeKind::Named { path, args } => TypeExpr::new(
            Span::default(),
            TypeExprKind::Named {
                path: Path::new(path.clone()),
                args: args.iter().map(type_expr_from_inferred_type).collect(),
            },
        ),
        InferredTypeKind::Tuple(items) => TypeExpr::new(
            Span::default(),
            TypeExprKind::Tuple(items.iter().map(type_expr_from_inferred_type).collect()),
        ),
        InferredTypeKind::Callable { params, ret } => TypeExpr::new(
            Span::default(),
            TypeExprKind::Callable {
                params: params.iter().map(type_expr_from_inferred_type).collect(),
                ret: Box::new(type_expr_from_inferred_type(ret)),
            },
        ),
    }
}

fn parse_rendered_type_substitution(rendered: &str) -> Option<TypeExpr> {
    let source = format!("fn __ql_type_probe(value: {rendered}) -> Int {{ return 0 }}");
    let module = ql_parser::parse_source(&source).ok()?;
    module.items.into_iter().find_map(|item| {
        let ItemKind::Function(function) = item.kind else {
            return None;
        };
        function.params.into_iter().find_map(|param| match param {
            Param::Regular { ty, .. } => Some(ty),
            Param::Receiver { .. } => None,
        })
    })
}
