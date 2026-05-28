use ql_ast::{TypeExpr, TypeExprKind};

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
mod inferred_types_tests;
