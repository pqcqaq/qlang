use super::inferred_types::{InferredType, InferredTypeKind};

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

fn is_inferred_string_type(ty: &InferredType) -> bool {
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
