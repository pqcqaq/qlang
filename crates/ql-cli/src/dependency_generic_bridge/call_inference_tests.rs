use ql_ast::{ExprKind, FunctionDecl, ItemKind, Module, StmtKind};

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
fn infers_array_type_and_length_substitutions_from_call_arguments() {
    let dependency = parse_module(
        r#"
package dep

pub fn reverse[T, N](values: [T; N]) -> [T; N] {
    return values
}
"#,
    );
    let root = parse_module(
        r#"
use dep.reverse as reverse

fn hidden() -> Int {
    return 2
}

fn run() -> [Int; 3] {
    return reverse([1, hidden(), 3])
}
"#,
    );
    let run = function(&root, "run");
    let StmtKind::Return(Some(expr)) = &run
        .body
        .as_ref()
        .expect("run should have a body")
        .statements[0]
        .kind
    else {
        panic!("run should return a call");
    };
    let ExprKind::Call { args, .. } = &expr.kind else {
        panic!("return expression should be a call");
    };

    let substitutions = infer_dependency_generic_function_substitutions(
        function(&dependency, "reverse"),
        args,
        run.return_type.as_ref(),
        &ValueTypeBindings::new(),
        &FunctionTypeBindings::new(),
    )
    .expect("call substitutions should infer");

    assert_eq!(substitutions.get("T").map(String::as_str), Some("Int"));
    assert_eq!(substitutions.get("N").map(String::as_str), Some("3"));
}

#[test]
fn orders_named_call_expected_types_after_return_substitutions() {
    let dependency = parse_module(
        r#"
package dep

pub fn pair[A, B](first: A, second: B) -> (A, B) {
    return (first, second)
}
"#,
    );
    let root = parse_module(
        r#"
use dep.pair as pair

fn run() -> (Int, Bool) {
    return pair(second: true, first: 1)
}
"#,
    );
    let run = function(&root, "run");
    let StmtKind::Return(Some(expr)) = &run
        .body
        .as_ref()
        .expect("run should have a body")
        .statements[0]
        .kind
    else {
        panic!("run should return a call");
    };
    let ExprKind::Call { callee, args } = &expr.kind else {
        panic!("return expression should be a call");
    };
    let function_bindings =
        FunctionTypeBindings::from([("pair".to_owned(), function(&dependency, "pair").clone())]);

    let expected_types = ordered_call_arg_expected_types(
        callee,
        args,
        run.return_type.as_ref(),
        &ValueTypeBindings::new(),
        &function_bindings,
    );

    let rendered = expected_types
        .iter()
        .map(|ty| {
            ty.as_ref()
                .and_then(InferredType::from_type_expr)
                .map(|ty| ty.rendered)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        rendered,
        vec![Some("Bool".to_owned()), Some("Int".to_owned())]
    );
}

#[test]
fn infers_distinct_substitutions_from_reversed_named_arguments() {
    let dependency = parse_module(
        r#"
package dep

pub fn pair[A, B](first: A, second: B) -> (A, B) {
    return (first, second)
}
"#,
    );
    let root = parse_module(
        r#"
use dep.pair as pair

fn run() -> (Int, Bool) {
    return pair(second: true, first: 1)
}
"#,
    );
    let run = function(&root, "run");
    let StmtKind::Return(Some(expr)) = &run
        .body
        .as_ref()
        .expect("run should have a body")
        .statements[0]
        .kind
    else {
        panic!("run should return a call");
    };
    let ExprKind::Call { args, .. } = &expr.kind else {
        panic!("return expression should be a call");
    };

    let substitutions = infer_dependency_generic_function_substitutions(
        function(&dependency, "pair"),
        args,
        run.return_type.as_ref(),
        &ValueTypeBindings::new(),
        &FunctionTypeBindings::new(),
    )
    .expect("reversed named arguments should infer");

    assert_eq!(substitutions.get("A").map(String::as_str), Some("Int"));
    assert_eq!(substitutions.get("B").map(String::as_str), Some("Bool"));
}
