use std::collections::BTreeSet;

use ql_ast::{ItemKind, Param, TypeExpr};

use super::super::inferred_types::InferredType;
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
