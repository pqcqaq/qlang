mod support;

use ql_hir::{ExprKind, PatternKind, StmtKind};
use ql_resolve::ValueResolution;

use support::{find_function, find_impl_method, find_item_id, path, resolved};

#[test]
fn resolves_function_parameters_in_body_uses() {
    let (module, resolution) = resolved(
        r#"
fn identity(value: Int) -> Int {
    value
}
"#,
    );

    let function = find_function(&module, "identity");
    let body = module.block(function.body.expect("function should have body"));
    let tail = body
        .tail
        .expect("function body should have tail expression");

    assert!(
        matches!(
            resolution.expr_resolution(tail),
            Some(ValueResolution::Param(binding)) if binding.index == 0
        ),
        "tail expression should resolve to the first function parameter"
    );
}

#[test]
fn local_bindings_shadow_outer_scopes() {
    let (module, resolution) = resolved(
        r#"
fn demo(value: Int) -> Int {
    let value = 1
    {
        let value = 2
        value
    };
    value
}
"#,
    );

    let function = find_function(&module, "demo");
    let body = module.block(function.body.expect("function should have body"));

    let StmtKind::Let { pattern, .. } = &module.stmt(body.statements[0]).kind else {
        panic!("first statement should be a let binding");
    };
    let PatternKind::Binding(outer_local) = &module.pattern(*pattern).kind else {
        panic!("outer let should bind a local");
    };

    let StmtKind::Expr { expr, .. } = &module.stmt(body.statements[1]).kind else {
        panic!("second statement should be a block expression");
    };
    let ExprKind::Block(inner_block_id) = &module.expr(*expr).kind else {
        panic!("second statement should wrap an inner block");
    };
    let inner_block = module.block(*inner_block_id);
    let StmtKind::Let { pattern, .. } = &module.stmt(inner_block.statements[0]).kind else {
        panic!("inner block should start with a let binding");
    };
    let PatternKind::Binding(inner_local) = &module.pattern(*pattern).kind else {
        panic!("inner let should bind a local");
    };

    let inner_tail = inner_block
        .tail
        .expect("inner block should have a tail use");
    let outer_tail = body.tail.expect("outer body should have a tail use");

    assert!(
        matches!(
            resolution.expr_resolution(inner_tail),
            Some(ValueResolution::Local(local_id)) if *local_id == *inner_local
        ),
        "inner tail should resolve to the inner shadowing binding"
    );
    assert!(
        matches!(
            resolution.expr_resolution(outer_tail),
            Some(ValueResolution::Local(local_id)) if *local_id == *outer_local
        ),
        "outer tail should resolve to the outer local binding"
    );
}

#[test]
fn resolves_closure_parameters() {
    let (module, resolution) = resolved(
        r#"
fn demo() -> Int {
    let closure = (item) => item
    0
}
"#,
    );

    let function = find_function(&module, "demo");
    let body = module.block(function.body.expect("function should have body"));
    let StmtKind::Let { value, .. } = &module.stmt(body.statements[0]).kind else {
        panic!("first statement should bind the closure");
    };
    let ExprKind::Closure { params, body, .. } = &module.expr(*value).kind else {
        panic!("let initializer should be a closure");
    };

    assert!(
        matches!(
            resolution.expr_resolution(*body),
            Some(ValueResolution::Local(local_id)) if *local_id == params[0]
        ),
        "closure body should resolve to its first parameter"
    );
}

#[test]
fn resolves_self_inside_method_receiver_scope() {
    let (module, resolution) = resolved(
        r#"
struct Counter {
    value: Int,
}

impl Counter {
    fn read(self) -> Int {
        self.value
    }
}
"#,
    );

    let method = find_impl_method(&module, "read");
    let body = module.block(method.body.expect("method should have body"));
    let tail = body
        .tail
        .expect("method body should have a tail expression");
    let ExprKind::Member { object, .. } = &module.expr(tail).kind else {
        panic!("method tail should be a member access");
    };

    assert_eq!(
        resolution.expr_resolution(*object),
        Some(&ValueResolution::SelfValue),
        "`self` should resolve inside methods with a receiver"
    );
    assert_eq!(
        resolution.expr_resolution(tail),
        Some(&ValueResolution::SelfValue),
        "path-like member access should keep the resolved root binding"
    );
}

#[test]
fn resolves_pattern_paths_to_their_root_item() {
    let (module, resolution) = resolved(
        r#"
enum Command {
    Quit,
    Retry(Int),
}

fn classify(command: Command) -> Int {
    match command {
        Command.Retry(times) => times,
        _ => 0,
    }
}
"#,
    );

    let command_item = find_item_id(&module, "Command");
    let function = find_function(&module, "classify");
    let body = module.block(function.body.expect("function should have body"));
    let tail = body
        .tail
        .expect("function body should have a tail expression");
    let ExprKind::Match { arms, .. } = &module.expr(tail).kind else {
        panic!("function tail should be a match expression");
    };

    assert_eq!(
        resolution.pattern_resolution(arms[0].pattern),
        Some(&ValueResolution::Item(command_item))
    );
    assert_eq!(
        resolution.expr_resolution(arms[0].body),
        Some(&ValueResolution::Local(
            match &module.pattern(arms[0].pattern).kind {
                PatternKind::TupleStruct { items, .. } => match &module.pattern(items[0]).kind {
                    PatternKind::Binding(local_id) => *local_id,
                    _ => panic!("variant payload should bind a local"),
                },
                _ => panic!("first arm should be a tuple-struct pattern"),
            }
        ))
    );
}

#[test]
fn resolves_import_aliases_in_expression_position() {
    let (module, resolution) = resolved(
        r#"
use std.collections.HashMap as Map

fn factory() -> Int {
    Map
}
"#,
    );

    let function = find_function(&module, "factory");
    let body = module.block(function.body.expect("function should have body"));
    let tail = body
        .tail
        .expect("function body should have a tail expression");

    assert_eq!(
        resolution.expr_resolution(tail),
        Some(&ValueResolution::Import(path(&[
            "std",
            "collections",
            "HashMap"
        ])))
    );
}
