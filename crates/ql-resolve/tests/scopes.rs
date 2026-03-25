mod support;

use ql_hir::{ExprKind, StmtKind};
use ql_resolve::ScopeKind;

use support::{find_function, find_impl_method, find_item_id, resolved};

#[test]
fn for_loops_insert_a_binding_scope_between_parent_and_body_block() {
    let (module, resolution) = resolved(
        r#"
fn walk() -> Int {
    for item in [1, 2, 3] {
        item
    }
    0
}
"#,
    );

    let function = find_function(&module, "walk");
    let body_id = function.body.expect("function should have body");
    let body = module.block(body_id);
    let parent_block_scope = resolution
        .block_scope(body_id)
        .expect("function body should have a scope");

    let StmtKind::For { body, .. } = &module.stmt(body.statements[0]).kind else {
        panic!("first statement should be a for loop");
    };
    let loop_body_scope = resolution
        .block_scope(*body)
        .expect("for body block should have its own scope");
    let loop_scope = resolution
        .scopes
        .scope(loop_body_scope)
        .parent
        .expect("for body scope should have a parent");

    assert_eq!(resolution.scopes.scope(loop_scope).kind, ScopeKind::ForLoop);
    assert_eq!(
        resolution.scopes.scope(loop_scope).parent,
        Some(parent_block_scope)
    );
}

#[test]
fn match_arms_share_a_dedicated_arm_scope_for_pattern_guard_and_body() {
    let (module, resolution) = resolved(
        r#"
enum Command {
    Retry(Int),
}

fn classify(command: Command) -> Int {
    match command {
        Command.Retry(times) if times > 0 => times,
        _ => 0,
    }
}
"#,
    );

    let function = find_function(&module, "classify");
    let body_id = function.body.expect("function should have body");
    let body = module.block(body_id);
    let match_expr = body.tail.expect("function body should end with a match");
    let parent_scope = resolution
        .block_scope(body_id)
        .expect("function body block should have a scope");
    let ExprKind::Match { arms, .. } = &module.expr(match_expr).kind else {
        panic!("function tail should be a match expression");
    };
    let first_arm = &arms[0];
    let guard = first_arm.guard.expect("first arm should have a guard");

    let pattern_scope = resolution
        .pattern_scope(first_arm.pattern)
        .expect("pattern should record its scope");
    let guard_scope = resolution
        .expr_scope(guard)
        .expect("guard should record its scope");
    let body_scope = resolution
        .expr_scope(first_arm.body)
        .expect("arm body should record its scope");

    assert_eq!(pattern_scope, guard_scope);
    assert_eq!(guard_scope, body_scope);
    assert_eq!(
        resolution.scopes.scope(pattern_scope).kind,
        ScopeKind::MatchArm
    );
    assert_eq!(
        resolution.scopes.scope(pattern_scope).parent,
        Some(parent_scope)
    );
}

#[test]
fn records_item_and_function_scopes_for_query_layers() {
    let (module, resolution) = resolved(
        r#"
struct Counter {
    value: Int,
}

fn make() -> Counter {
    Counter { value: 1 }
}

impl Counter {
    fn read(self) -> Int {
        self.value
    }
}
"#,
    );

    let counter_item = find_item_id(&module, "Counter");
    let make_item = find_item_id(&module, "make");
    let make_function = find_function(&module, "make");
    let make_body = make_function.body.expect("function should have a body");
    let make_body_scope = resolution
        .block_scope(make_body)
        .expect("function body should have a scope");
    let make_scope = resolution
        .item_scope(make_item)
        .expect("function item should record its scope");
    let method = find_impl_method(&module, "read");

    assert_eq!(
        resolution.function_scope(make_function.span),
        Some(make_scope)
    );
    assert_eq!(
        resolution.scopes.scope(make_body_scope).parent,
        Some(make_scope)
    );
    assert!(
        resolution.item_scope(counter_item).is_some(),
        "top-level named items should record their item scope"
    );
    assert!(
        resolution.function_scope(method.span).is_some(),
        "methods should record a dedicated function scope even without an ItemId"
    );
}
