use ql_ast::{Expr, ExprKind, FunctionDecl, ItemKind, Module};

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
    )
    .expect("single-field generic variant call should infer");

    assert_eq!(choose_ty.rendered, "Int");
    assert_eq!(wrap_ty.rendered, "Option[Int]");
}
