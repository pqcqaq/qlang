use std::collections::BTreeMap;

use ql_ast::{ItemKind, Param, TypeExpr, TypeExprKind};
use ql_span::Span;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct InferredType {
    pub(super) rendered: String,
    pub(super) kind: InferredTypeKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum InferredTypeKind {
    Pointer {
        is_const: bool,
        inner: Box<InferredType>,
    },
    Array {
        element: Box<InferredType>,
        len: String,
    },
    Named {
        path: Vec<String>,
        args: Vec<InferredType>,
    },
    Tuple(Vec<InferredType>),
    Callable {
        params: Vec<InferredType>,
        ret: Box<InferredType>,
    },
}

impl InferredType {
    pub(super) fn primitive(name: &str) -> Self {
        Self {
            rendered: name.to_owned(),
            kind: InferredTypeKind::Named {
                path: vec![name.to_owned()],
                args: Vec::new(),
            },
        }
    }

    pub(super) fn from_type_expr(ty: &TypeExpr) -> Option<Self> {
        match &ty.kind {
            TypeExprKind::Named { path, args } => {
                let args = args
                    .iter()
                    .map(Self::from_type_expr)
                    .collect::<Option<Vec<_>>>()?;
                Some(Self {
                    rendered: render_inferred_named_type(&path.segments, &args),
                    kind: InferredTypeKind::Named {
                        path: path.segments.clone(),
                        args,
                    },
                })
            }
            TypeExprKind::Tuple(items) => {
                let items = items
                    .iter()
                    .map(Self::from_type_expr)
                    .collect::<Option<Vec<_>>>()?;
                Some(Self {
                    rendered: render_inferred_tuple_type(&items),
                    kind: InferredTypeKind::Tuple(items),
                })
            }
            TypeExprKind::Array { element, len } => {
                let element = Self::from_type_expr(element)?;
                Some(Self::array(element, len.clone()))
            }
            TypeExprKind::Pointer { is_const, inner } => {
                let inner = Self::from_type_expr(inner)?;
                let qualifier = if *is_const { "const " } else { "" };
                Some(Self {
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
                    .map(Self::from_type_expr)
                    .collect::<Option<Vec<_>>>()?;
                let ret = Self::from_type_expr(ret)?;
                Some(Self::callable(params, ret))
            }
        }
    }

    pub(super) fn array(element: InferredType, len: String) -> Self {
        Self {
            rendered: format!("[{}; {len}]", element.rendered),
            kind: InferredTypeKind::Array {
                element: Box::new(element),
                len,
            },
        }
    }

    pub(super) fn named(path: Vec<String>, args: Vec<InferredType>) -> Self {
        Self {
            rendered: render_inferred_named_type(&path, &args),
            kind: InferredTypeKind::Named { path, args },
        }
    }

    pub(super) fn callable(params: Vec<InferredType>, ret: InferredType) -> Self {
        Self {
            rendered: format!(
                "({}) -> {}",
                params
                    .iter()
                    .map(|param| param.rendered.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
                ret.rendered
            ),
            kind: InferredTypeKind::Callable {
                params,
                ret: Box::new(ret),
            },
        }
    }
}

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
                path: ql_ast::Path::new(path.clone()),
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

pub(super) fn render_inferred_tuple_type(items: &[InferredType]) -> String {
    let mut rendered = String::from("(");
    rendered.push_str(
        &items
            .iter()
            .map(|item| item.rendered.as_str())
            .collect::<Vec<_>>()
            .join(", "),
    );
    if items.len() == 1 {
        rendered.push(',');
    }
    rendered.push(')');
    rendered
}

pub(super) fn are_inferred_bool_types(left: &InferredType, right: &InferredType) -> bool {
    is_inferred_bool_type(left) && is_inferred_bool_type(right)
}

pub(super) fn is_inferred_bool_type(ty: &InferredType) -> bool {
    is_inferred_builtin_type(ty, "Bool")
}

pub(super) fn is_inferred_equality_comparable_type(ty: &InferredType) -> bool {
    is_inferred_numeric_type(ty) || is_inferred_bool_type(ty) || is_inferred_string_type(ty)
}

pub(super) fn is_inferred_ordered_comparable_type(ty: &InferredType) -> bool {
    is_inferred_numeric_type(ty) || is_inferred_string_type(ty)
}

pub(super) fn is_inferred_string_type(ty: &InferredType) -> bool {
    is_inferred_builtin_type(ty, "String")
}

pub(super) fn is_inferred_numeric_type(ty: &InferredType) -> bool {
    matches!(
        inferred_named_builtin_type(ty),
        Some(
            "Int"
                | "UInt"
                | "I8"
                | "I16"
                | "I32"
                | "I64"
                | "ISize"
                | "U8"
                | "U16"
                | "U32"
                | "U64"
                | "USize"
                | "F32"
                | "F64"
        )
    )
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

fn is_inferred_builtin_type(ty: &InferredType, expected: &str) -> bool {
    inferred_named_builtin_type(ty) == Some(expected)
}

fn inferred_named_builtin_type(ty: &InferredType) -> Option<&str> {
    let InferredTypeKind::Named { path, args } = &ty.kind else {
        return None;
    };
    if !args.is_empty() || path.len() != 1 {
        return None;
    }
    path.first().map(String::as_str)
}

fn render_inferred_named_type(path: &[String], args: &[InferredType]) -> String {
    let mut rendered = path.join(".");
    if !args.is_empty() {
        rendered.push('[');
        rendered.push_str(
            &args
                .iter()
                .map(|arg| arg.rendered.as_str())
                .collect::<Vec<_>>()
                .join(", "),
        );
        rendered.push(']');
    }
    rendered
}

#[cfg(test)]
#[path = "inferred_types_tests.rs"]
mod tests;
