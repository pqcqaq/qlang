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
