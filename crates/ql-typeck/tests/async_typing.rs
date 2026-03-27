mod support;

use support::diagnostic_messages;

#[test]
fn reports_await_outside_async_functions() {
    let diagnostics = diagnostic_messages(
        r#"
fn worker() -> Int {
    return 1
}

fn main() -> Int {
    return await worker()
}
"#,
    );

    assert!(
        diagnostics.contains(&"`await` is only allowed inside `async fn`".to_string()),
        "expected async-boundary diagnostic, got {diagnostics:?}"
    );
}

#[test]
fn reports_spawn_outside_async_functions() {
    let diagnostics = diagnostic_messages(
        r#"
fn worker() -> Int {
    return 1
}

fn main() -> Int {
    spawn worker()
    return 0
}
"#,
    );

    assert!(
        diagnostics.contains(&"`spawn` is only allowed inside `async fn`".to_string()),
        "expected async-boundary diagnostic, got {diagnostics:?}"
    );
}

#[test]
fn allows_await_and_spawn_async_calls_inside_async_functions() {
    let diagnostics = diagnostic_messages(
        r#"
async fn worker() -> Int {
    return 1
}

async fn main() -> Int {
    spawn worker()
    return await worker()
}
"#,
    );

    assert!(
        diagnostics
            .iter()
            .all(|message| !message.contains("only allowed inside `async fn`")),
        "did not expect async-boundary diagnostics in async function, got {diagnostics:?}"
    );
    assert!(
        diagnostics
            .iter()
            .all(|message| !message.contains("must be consumed by `await` or `spawn`")),
        "did not expect direct async-call diagnostics for await/spawn operands, got {diagnostics:?}"
    );
}

#[test]
fn allows_awaiting_spawned_task_handles_inside_async_functions() {
    let diagnostics = diagnostic_messages(
        r#"
async fn worker() -> Int {
    return 1
}

async fn main() -> Int {
    let task = spawn worker()
    return await task
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected awaiting a spawned task handle to succeed, got {diagnostics:?}"
    );
}

#[test]
fn allows_spawning_bound_task_handles_inside_async_functions() {
    let diagnostics = diagnostic_messages(
        r#"
async fn worker() -> Int {
    return 1
}

async fn main() -> Int {
    let task = worker()
    let running = spawn task
    return await running
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected bound task handles to be spawnable, got {diagnostics:?}"
    );
}

#[test]
fn allows_binding_direct_async_call_handles_before_awaiting() {
    let diagnostics = diagnostic_messages(
        r#"
async fn worker() -> Int {
    return 1
}

async fn main() -> Int {
    let task = worker()
    return await task
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected direct async-call handles to be bindable before await, got {diagnostics:?}"
    );
}

#[test]
fn reports_direct_async_calls_as_task_handle_type_mismatches_when_not_awaited() {
    let diagnostics = diagnostic_messages(
        r#"
async fn worker() -> Int {
    return 1
}

fn main() -> Int {
    return worker()
}
"#,
    );

    assert!(
        diagnostics.contains(
            &"return value has type mismatch: expected `Int`, found `Task[Int]`".to_string()
        ),
        "expected direct async-call to surface as task-handle mismatch, got {diagnostics:?}"
    );
}

#[test]
fn accepts_returning_async_task_handles_through_explicit_task_types() {
    let diagnostics = diagnostic_messages(
        r#"
async fn worker() -> Int {
    return 1
}

fn schedule() -> Task[Int] {
    return worker()
}

async fn main() -> Int {
    return await schedule()
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected explicit Task-return helper flow to succeed, got {diagnostics:?}"
    );
}

#[test]
fn allows_spawning_task_handle_helpers_inside_async_functions() {
    let diagnostics = diagnostic_messages(
        r#"
async fn worker() -> Int {
    return 1
}

fn schedule() -> Task[Int] {
    return worker()
}

async fn main() -> Int {
    let task = spawn schedule()
    return await task
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected task-handle helper calls to be spawnable, got {diagnostics:?}"
    );
}

#[test]
fn accepts_passing_async_calls_to_task_handle_parameters() {
    let diagnostics = diagnostic_messages(
        r#"
async fn worker() -> Int {
    return 1
}

fn forward(task: Task[Int]) -> Task[Int] {
    return task
}

async fn main() -> Int {
    return await forward(worker())
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected task-handle parameter flow to succeed, got {diagnostics:?}"
    );
}

#[test]
fn reports_await_sync_calls_inside_async_functions() {
    let diagnostics = diagnostic_messages(
        r#"
fn worker() -> Int {
    return 1
}

async fn main() -> Int {
    return await worker()
}
"#,
    );

    assert!(
        diagnostics.contains(&"`await` currently requires calling an `async fn`".to_string()),
        "expected await async-call-target diagnostic, got {diagnostics:?}"
    );
}

#[test]
fn reports_spawn_sync_calls_inside_async_functions() {
    let diagnostics = diagnostic_messages(
        r#"
fn worker() -> Int {
    return 1
}

async fn main() -> Int {
    spawn worker()
    return 0
}
"#,
    );

    assert!(
        diagnostics.contains(
            &"`spawn` currently requires calling an `async fn` or task-handle helper".to_string()
        ),
        "expected spawn async-call-target diagnostic, got {diagnostics:?}"
    );
}

#[test]
fn reports_await_non_call_operand_inside_async_functions() {
    let diagnostics = diagnostic_messages(
        r#"
fn worker() -> Int {
    return 1
}

async fn main() -> Int {
    let value = worker()
    return await value
}
"#,
    );

    assert!(
        diagnostics
            .contains(&"`await` currently requires an async task handle operand".to_string()),
        "expected await task-handle diagnostic, got {diagnostics:?}"
    );
}

#[test]
fn reports_spawn_non_call_operand_inside_async_functions() {
    let diagnostics = diagnostic_messages(
        r#"
fn worker() -> Int {
    return 1
}

async fn main() -> Int {
    let value = worker()
    spawn value
    return 0
}
"#,
    );

    assert!(
        diagnostics
            .contains(&"`spawn` currently requires an async task handle operand".to_string()),
        "expected spawn task-handle diagnostic, got {diagnostics:?}"
    );
}

#[test]
fn reports_await_closure_calls_inside_async_functions() {
    let diagnostics = diagnostic_messages(
        r#"
async fn main() -> Int {
    let run = () => 1
    return await run()
}
"#,
    );

    assert!(
        diagnostics.contains(&"`await` currently requires calling an `async fn`".to_string()),
        "expected closure await async-call-target diagnostic, got {diagnostics:?}"
    );
}

#[test]
fn renders_spawned_task_handle_types_in_return_mismatches() {
    let diagnostics = diagnostic_messages(
        r#"
async fn worker() -> Int {
    return 1
}

async fn main() -> Int {
    let task = spawn worker()
    return task
}
"#,
    );

    assert!(
        diagnostics.contains(
            &"return value has type mismatch: expected `Int`, found `Task[Int]`".to_string()
        ),
        "expected spawned task handle type in mismatch, got {diagnostics:?}"
    );
}

#[test]
fn reports_spawn_closure_calls_inside_async_functions() {
    let diagnostics = diagnostic_messages(
        r#"
async fn main() -> Int {
    let run = () => 1
    spawn run()
    return 0
}
"#,
    );

    assert!(
        diagnostics.contains(
            &"`spawn` currently requires calling an `async fn` or task-handle helper".to_string()
        ),
        "expected closure spawn async-call-target diagnostic, got {diagnostics:?}"
    );
}

#[test]
fn reports_for_await_outside_async_functions() {
    let diagnostics = diagnostic_messages(
        r#"
fn main() -> Int {
    for await value in [1, 2, 3] {
        let current = value
    }
    return 0
}
"#,
    );

    assert!(
        diagnostics.contains(&"`for await` is only allowed inside `async fn`".to_string()),
        "expected async-boundary diagnostic, got {diagnostics:?}"
    );
}

#[test]
fn allows_for_await_inside_async_functions() {
    let diagnostics = diagnostic_messages(
        r#"
async fn main() -> Int {
    for await value in [1, 2, 3] {
        let current = value
    }
    return 0
}
"#,
    );

    assert!(
        diagnostics
            .iter()
            .all(|message| !message.contains("`for await` is only allowed inside `async fn`")),
        "did not expect for-await boundary diagnostics in async function, got {diagnostics:?}"
    );
}

#[test]
fn allows_await_and_spawn_import_aliased_async_calls_inside_async_functions() {
    let diagnostics = diagnostic_messages(
        r#"
use worker as run

async fn worker() -> Int {
    return 1
}

async fn main() -> Int {
    spawn run()
    return await run()
}
"#,
    );

    assert!(
        diagnostics
            .iter()
            .all(|message| !message.contains("currently requires calling an `async fn`")),
        "did not expect async-call-target diagnostics for imported async calls, got {diagnostics:?}"
    );
}

#[test]
fn async_method_calls_require_async_targets() {
    let diagnostics = diagnostic_messages(
        r#"
struct Counter {
    value: Int,
}

impl Counter {
    fn sync_run(self) -> Int {
        return self.value
    }

    async fn async_run(self) -> Int {
        return self.value
    }
}

async fn main(counter: Counter) -> Int {
    spawn counter.async_run()
    let task = spawn counter.sync_run()
    return await counter.sync_run()
}
"#,
    );

    assert!(
        diagnostics.contains(
            &"`spawn` currently requires calling an `async fn` or task-handle helper".to_string()
        ),
        "expected sync method spawn async-call-target diagnostic, got {diagnostics:?}"
    );
    assert!(
        diagnostics.contains(&"`await` currently requires calling an `async fn`".to_string()),
        "expected sync method await async-call-target diagnostic, got {diagnostics:?}"
    );
    assert_eq!(
        diagnostics
            .iter()
            .filter(|message| message.contains("currently requires calling an `async fn`"))
            .count(),
        2,
        "expected only sync method calls to violate async-call-target rules, got {diagnostics:?}"
    );
}

#[test]
fn async_method_boundaries_follow_impl_and_extend_contexts() {
    let diagnostics = diagnostic_messages(
        r#"
struct Counter {
    value: Int,
}

async fn worker() -> Int {
    return 1
}

impl Counter {
    fn sync_run(self) -> Int {
        spawn worker()
        return await worker()
    }

    async fn async_run(self) -> Int {
        spawn worker()
        return await worker()
    }
}

extend Counter {
    fn sync_stream(self) -> Int {
        for await value in [1, 2, 3] {
            let current = value
        }
        return 0
    }

    async fn async_stream(self) -> Int {
        for await value in [1, 2, 3] {
            let current = value
        }
        return 0
    }
}
"#,
    );

    assert!(
        diagnostics.contains(&"`spawn` is only allowed inside `async fn`".to_string()),
        "expected sync impl method spawn boundary diagnostic, got {diagnostics:?}"
    );
    assert!(
        diagnostics.contains(&"`await` is only allowed inside `async fn`".to_string()),
        "expected sync impl method await boundary diagnostic, got {diagnostics:?}"
    );
    assert!(
        diagnostics.contains(&"`for await` is only allowed inside `async fn`".to_string()),
        "expected sync extend method for-await boundary diagnostic, got {diagnostics:?}"
    );
    assert_eq!(
        diagnostics
            .iter()
            .filter(|message| message.contains("only allowed inside `async fn`"))
            .count(),
        3,
        "expected only the sync methods to contribute async-boundary diagnostics, got {diagnostics:?}"
    );
}

#[test]
fn async_trait_method_boundaries_follow_default_method_contexts() {
    let diagnostics = diagnostic_messages(
        r#"
async fn worker() -> Int {
    return 1
}

trait Runner {
    fn sync_run(self) -> Int {
        spawn worker()
        return await worker()
    }

    async fn async_run(self) -> Int {
        spawn worker()
        return await worker()
    }

    fn sync_stream(self) -> Int {
        for await value in [1, 2, 3] {
            let current = value
        }
        return 0
    }

    async fn async_stream(self) -> Int {
        for await value in [1, 2, 3] {
            let current = value
        }
        return 0
    }
}
"#,
    );

    assert!(
        diagnostics.contains(&"`spawn` is only allowed inside `async fn`".to_string()),
        "expected sync trait method spawn boundary diagnostic, got {diagnostics:?}"
    );
    assert!(
        diagnostics.contains(&"`await` is only allowed inside `async fn`".to_string()),
        "expected sync trait method await boundary diagnostic, got {diagnostics:?}"
    );
    assert!(
        diagnostics.contains(&"`for await` is only allowed inside `async fn`".to_string()),
        "expected sync trait method for-await boundary diagnostic, got {diagnostics:?}"
    );
    assert_eq!(
        diagnostics
            .iter()
            .filter(|message| message.contains("only allowed inside `async fn`"))
            .count(),
        3,
        "expected only the sync trait methods to contribute async-boundary diagnostics, got {diagnostics:?}"
    );
}

#[test]
fn closures_do_not_inherit_async_function_boundaries() {
    let diagnostics = diagnostic_messages(
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

    assert!(
        diagnostics.contains(&"`spawn` is only allowed inside `async fn`".to_string()),
        "expected closure spawn boundary diagnostic, got {diagnostics:?}"
    );
    assert!(
        diagnostics.contains(&"`await` is only allowed inside `async fn`".to_string()),
        "expected closure await boundary diagnostic, got {diagnostics:?}"
    );
    assert!(
        diagnostics.contains(&"`for await` is only allowed inside `async fn`".to_string()),
        "expected closure for-await boundary diagnostic, got {diagnostics:?}"
    );
    assert_eq!(
        diagnostics
            .iter()
            .filter(|message| message.contains("only allowed inside `async fn`"))
            .count(),
        3,
        "expected closure body to contribute exactly three async-boundary diagnostics, got {diagnostics:?}"
    );
}
