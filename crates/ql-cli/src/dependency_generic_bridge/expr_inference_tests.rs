use ql_ast::{Expr, ExprKind, FunctionDecl, ItemKind, Module};

use super::super::enum_bindings::collect_local_enum_type_bindings;
use super::*;

fn parse_module(source: &str) -> Module {
    ql_parser::parse_source(source).expect("test source should parse")
}

fn function<'a>(module: &'a Module, name: &str) -> &'a FunctionDecl {
    module
        .items
        .iter()
        .find_map(|item| match &item.kind {
            ItemKind::Function(function) if function.name == name => Some(function),
            _ => None,
        })
        .expect("test function should exist")
}

#[test]
fn infers_block_tail_from_local_bindings_and_projection() {
    let module = parse_module(
        r#"
fn run() -> Int {
    let values: [Int; 3] = [1, 2, 3]
    let pair: (Int, Bool) = (values[0], true)
    {
        let local = pair[0]
        local
    }
}
"#,
    );
    let body = function(&module, "run")
        .body
        .as_ref()
        .expect("function should have a body")
        .clone();
    let expr = Expr::new(body.span, ExprKind::Block(body));

    let inferred = infer_dependency_generic_expr_type(
        &expr,
        &ValueTypeBindings::new(),
        &FunctionTypeBindings::new(),
        &collect_local_enum_type_bindings(&module),
    )
    .expect("block tail should infer");

    assert_eq!(inferred.rendered, "Int");
}

#[test]
fn infers_if_tail_and_single_field_generic_variant_call() {
    let module = parse_module(
        r#"
enum Option[T] {
    Some(T),
    None,
}

fn choose() -> Int {
    if true {
        1
    } else {
        2
    }
}

fn wrap() -> Option[Int] {
    Option.Some(42)
}
"#,
    );

    let choose_body = function(&module, "choose")
        .body
        .as_ref()
        .expect("choose should have a body")
        .clone();
    let choose_expr = Expr::new(choose_body.span, ExprKind::Block(choose_body));
    let choose_ty = infer_dependency_generic_expr_type(
        &choose_expr,
        &ValueTypeBindings::new(),
        &FunctionTypeBindings::new(),
        &collect_local_enum_type_bindings(&module),
    )
    .expect("if tail should infer");

    let wrap_body = function(&module, "wrap")
        .body
        .as_ref()
        .expect("wrap should have a body")
        .clone();
    let wrap_expr = Expr::new(wrap_body.span, ExprKind::Block(wrap_body));
    let wrap_ty = infer_dependency_generic_expr_type(
        &wrap_expr,
        &ValueTypeBindings::new(),
        &FunctionTypeBindings::new(),
        &collect_local_enum_type_bindings(&module),
    )
    .expect("single-field generic variant call should infer");

    assert_eq!(choose_ty.rendered, "Int");
    assert_eq!(wrap_ty.rendered, "Option[Int]");
}

#[test]
fn infers_generic_variant_call_from_enum_declaration() {
    let module = parse_module(
        r#"
enum PairBox[A, B] {
    Pair(A, B),
    Empty,
}

fn wrap() -> PairBox[Int, Bool] {
    PairBox.Pair(1, true)
}
"#,
    );
    let wrap_body = function(&module, "wrap")
        .body
        .as_ref()
        .expect("wrap should have a body")
        .clone();
    let wrap_expr = Expr::new(wrap_body.span, ExprKind::Block(wrap_body));

    let wrap_ty = infer_dependency_generic_expr_type(
        &wrap_expr,
        &ValueTypeBindings::new(),
        &FunctionTypeBindings::new(),
        &collect_local_enum_type_bindings(&module),
    )
    .expect("generic variant call should infer from enum declaration");

    assert_eq!(wrap_ty.rendered, "PairBox[Int, Bool]");
}

#[test]
fn infers_match_tail_with_generic_call_return_type() {
    let module = parse_module(
        r#"
fn identity[T](value: T) -> T {
    value
}

fn choose(flag: Bool) -> Int {
    let base: Int = 1
    match flag {
        true => identity(base + 1),
        _ => identity(2),
    }
}
"#,
    );
    let choose_body = function(&module, "choose")
        .body
        .as_ref()
        .expect("choose should have a body")
        .clone();
    let choose_expr = Expr::new(choose_body.span, ExprKind::Block(choose_body));
    let function_bindings = FunctionTypeBindings::from([(
        "identity".to_owned(),
        function(&module, "identity").clone(),
    )]);

    let choose_ty = infer_dependency_generic_expr_type(
        &choose_expr,
        &ValueTypeBindings::new(),
        &function_bindings,
        &collect_local_enum_type_bindings(&module),
    )
    .expect("match tail with generic call return type should infer");

    assert_eq!(choose_ty.rendered, "Int");
}

#[test]
fn infers_match_arm_body_from_tuple_pattern_bindings() {
    let module = parse_module(
        r#"
fn choose() -> Bool {
    match (1, true) {
        (number, flag) => flag,
        _ => false,
    }
}
"#,
    );
    let choose_body = function(&module, "choose")
        .body
        .as_ref()
        .expect("choose should have a body")
        .clone();
    let choose_expr = Expr::new(choose_body.span, ExprKind::Block(choose_body));

    let choose_ty = infer_dependency_generic_expr_type(
        &choose_expr,
        &ValueTypeBindings::new(),
        &FunctionTypeBindings::new(),
        &collect_local_enum_type_bindings(&module),
    )
    .expect("match arm pattern bindings should infer");

    assert_eq!(choose_ty.rendered, "Bool");
}
