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

#[test]
fn methods_in_the_same_impl_record_distinct_function_scopes() {
    let (module, resolution) = resolved(
        r#"
struct Counter {
    value: Int,
}

impl Counter {
    fn get(self) -> Int {
        return self.value
    }

    fn read(self) -> Int {
        return self.get()
    }
}
"#,
    );

    let impl_block = module
        .items
        .iter()
        .find_map(|&item_id| match &module.item(item_id).kind {
            ql_hir::ItemKind::Impl(impl_block) => Some(impl_block),
            _ => None,
        })
        .expect("impl block should exist");
    let get_scope = resolution
        .function_scope(impl_block.methods[0].span)
        .expect("first method should have a function scope");
    let read_scope = resolution
        .function_scope(impl_block.methods[1].span)
        .expect("second method should have a function scope");

    assert_ne!(impl_block.methods[0].span, impl_block.methods[1].span);
    assert_ne!(get_scope, read_scope);
}

#[test]
fn async_method_scopes_propagate_through_impl_and_extend_bodies() {
    let (module, resolution) = resolved(
        r#"
struct Counter {
    value: Int,
}

fn worker() -> Int {
    return 1
}

impl Counter {
    fn sync_run(self) -> Int {
        let value = await worker()
        return value
    }

    async fn async_run(self) -> Int {
        let value = await worker()
        return value
    }
}

extend Counter {
    fn sync_stream(self) -> Int {
        spawn worker()
        return 0
    }

    async fn async_stream(self) -> Int {
        spawn worker()
        return 0
    }
}
"#,
    );

    let impl_block = module
        .items
        .iter()
        .find_map(|&item_id| match &module.item(item_id).kind {
            ql_hir::ItemKind::Impl(impl_block) => Some(impl_block),
            _ => None,
        })
        .expect("impl block should exist");
    let extend_block = module
        .items
        .iter()
        .find_map(|&item_id| match &module.item(item_id).kind {
            ql_hir::ItemKind::Extend(extend_block) => Some(extend_block),
            _ => None,
        })
        .expect("extend block should exist");

    let sync_impl_scope = resolution
        .function_scope(impl_block.methods[0].span)
        .expect("sync impl method should have a scope");
    let async_impl_scope = resolution
        .function_scope(impl_block.methods[1].span)
        .expect("async impl method should have a scope");
    let sync_extend_scope = resolution
        .function_scope(extend_block.methods[0].span)
        .expect("sync extend method should have a scope");
    let async_extend_scope = resolution
        .function_scope(extend_block.methods[1].span)
        .expect("async extend method should have a scope");

    assert!(!resolution.scope_is_in_async_function(sync_impl_scope));
    assert!(resolution.scope_is_in_async_function(async_impl_scope));
    assert!(!resolution.scope_is_in_async_function(sync_extend_scope));
    assert!(resolution.scope_is_in_async_function(async_extend_scope));

    let sync_impl_body = module.block(
        impl_block.methods[0]
            .body
            .expect("sync impl method should have a body"),
    );
    let async_impl_body = module.block(
        impl_block.methods[1]
            .body
            .expect("async impl method should have a body"),
    );
    let sync_extend_body = module.block(
        extend_block.methods[0]
            .body
            .expect("sync extend method should have a body"),
    );
    let async_extend_body = module.block(
        extend_block.methods[1]
            .body
            .expect("async extend method should have a body"),
    );

    let StmtKind::Let {
        value: sync_impl_await,
        ..
    } = &module.stmt(sync_impl_body.statements[0]).kind
    else {
        panic!("sync impl method should start with a let binding");
    };
    let StmtKind::Let {
        value: async_impl_await,
        ..
    } = &module.stmt(async_impl_body.statements[0]).kind
    else {
        panic!("async impl method should start with a let binding");
    };
    let StmtKind::Expr {
        expr: sync_extend_spawn,
        ..
    } = &module.stmt(sync_extend_body.statements[0]).kind
    else {
        panic!("sync extend method should start with a spawn expression");
    };
    let StmtKind::Expr {
        expr: async_extend_spawn,
        ..
    } = &module.stmt(async_extend_body.statements[0]).kind
    else {
        panic!("async extend method should start with a spawn expression");
    };

    assert!(!resolution.expr_is_in_async_function(*sync_impl_await));
    assert!(resolution.expr_is_in_async_function(*async_impl_await));
    assert!(!resolution.expr_is_in_async_function(*sync_extend_spawn));
    assert!(resolution.expr_is_in_async_function(*async_extend_spawn));
}

#[test]
fn async_trait_method_scopes_propagate_through_default_bodies() {
    let (module, resolution) = resolved(
        r#"
fn worker() -> Int {
    return 1
}

trait Runner {
    fn sync_run(self) -> Int {
        let value = await worker()
        return value
    }

    async fn async_run(self) -> Int {
        let value = await worker()
        return value
    }

    fn sync_spawn(self) -> Int {
        spawn worker()
        return 0
    }

    async fn async_spawn(self) -> Int {
        spawn worker()
        return 0
    }
}
"#,
    );

    let trait_decl = module
        .items
        .iter()
        .find_map(|&item_id| match &module.item(item_id).kind {
            ql_hir::ItemKind::Trait(trait_decl) => Some(trait_decl),
            _ => None,
        })
        .expect("trait should exist");

    let sync_run_scope = resolution
        .function_scope(trait_decl.methods[0].span)
        .expect("sync trait method should have a scope");
    let async_run_scope = resolution
        .function_scope(trait_decl.methods[1].span)
        .expect("async trait method should have a scope");
    let sync_spawn_scope = resolution
        .function_scope(trait_decl.methods[2].span)
        .expect("sync trait spawn method should have a scope");
    let async_spawn_scope = resolution
        .function_scope(trait_decl.methods[3].span)
        .expect("async trait spawn method should have a scope");

    assert!(!resolution.scope_is_in_async_function(sync_run_scope));
    assert!(resolution.scope_is_in_async_function(async_run_scope));
    assert!(!resolution.scope_is_in_async_function(sync_spawn_scope));
    assert!(resolution.scope_is_in_async_function(async_spawn_scope));

    let sync_run_body = module.block(
        trait_decl.methods[0]
            .body
            .expect("sync trait method should have a body"),
    );
    let async_run_body = module.block(
        trait_decl.methods[1]
            .body
            .expect("async trait method should have a body"),
    );
    let sync_spawn_body = module.block(
        trait_decl.methods[2]
            .body
            .expect("sync trait spawn method should have a body"),
    );
    let async_spawn_body = module.block(
        trait_decl.methods[3]
            .body
            .expect("async trait spawn method should have a body"),
    );

    let StmtKind::Let {
        value: sync_run_await,
        ..
    } = &module.stmt(sync_run_body.statements[0]).kind
    else {
        panic!("sync trait method should start with a let binding");
    };
    let StmtKind::Let {
        value: async_run_await,
        ..
    } = &module.stmt(async_run_body.statements[0]).kind
    else {
        panic!("async trait method should start with a let binding");
    };
    let StmtKind::Expr {
        expr: sync_spawn_expr,
        ..
    } = &module.stmt(sync_spawn_body.statements[0]).kind
    else {
        panic!("sync trait spawn method should start with a spawn expression");
    };
    let StmtKind::Expr {
        expr: async_spawn_expr,
        ..
    } = &module.stmt(async_spawn_body.statements[0]).kind
    else {
        panic!("async trait spawn method should start with a spawn expression");
    };

    assert!(!resolution.expr_is_in_async_function(*sync_run_await));
    assert!(resolution.expr_is_in_async_function(*async_run_await));
    assert!(!resolution.expr_is_in_async_function(*sync_spawn_expr));
    assert!(resolution.expr_is_in_async_function(*async_spawn_expr));
}

#[test]
fn closure_scopes_break_async_function_context_in_queries() {
    let (module, resolution) = resolved(
        r#"
fn worker() -> Int {
    return 1
}

async fn main() -> Int {
    let runner = () => {
        for await value in [1, 2, 3] {
            let current = value
        }
        let job = spawn worker()
        await worker()
    }
    return 0
}
"#,
    );

    let main = find_function(&module, "main");
    let main_scope = resolution
        .function_scope(main.span)
        .expect("async main should have a function scope");
    assert!(resolution.scope_is_in_async_function(main_scope));

    let body = module.block(main.body.expect("main should have a body"));
    let StmtKind::Let { value: closure, .. } = &module.stmt(body.statements[0]).kind else {
        panic!("main should start with a closure binding");
    };
    let ExprKind::Closure {
        body: closure_body, ..
    } = &module.expr(*closure).kind
    else {
        panic!("first let binding should initialize a closure");
    };
    let ExprKind::Block(closure_block_id) = &module.expr(*closure_body).kind else {
        panic!("closure body should be a block expression");
    };
    let closure_scope = resolution
        .expr_scope(*closure_body)
        .expect("closure body should record its scope");
    assert!(
        !resolution.scope_is_in_async_function(closure_scope),
        "closure scope should break inherited async-function context"
    );

    let closure_block = module.block(*closure_block_id);
    let StmtKind::For {
        iterable: closure_iterable,
        ..
    } = &module.stmt(closure_block.statements[0]).kind
    else {
        panic!("closure should start with a for-await loop");
    };
    let StmtKind::Let {
        value: closure_spawn,
        ..
    } = &module.stmt(closure_block.statements[1]).kind
    else {
        panic!("closure should bind the spawn expression");
    };
    let closure_await = closure_block
        .tail
        .expect("closure block should end with an await expression");

    assert!(!resolution.expr_is_in_async_function(*closure_iterable));
    assert!(!resolution.expr_is_in_async_function(*closure_spawn));
    assert!(!resolution.expr_is_in_async_function(closure_await));
}
