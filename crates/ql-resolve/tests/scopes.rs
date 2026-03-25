mod support;

use ql_hir::{ExprKind, StmtKind};
use ql_resolve::ScopeKind;

use support::{find_function, resolved};

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
