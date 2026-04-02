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
fn accepts_awaiting_nested_task_handle_async_results() {
    let diagnostics = diagnostic_messages(
        r#"
async fn worker() -> Int {
    return 1
}

async fn outer() -> Task[Int] {
    return worker()
}

async fn main() -> Int {
    let next = await outer()
    return await next
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected nested task-handle async results to support chained await, got {diagnostics:?}"
    );
}

#[test]
fn accepts_awaiting_nested_zero_sized_task_handle_async_results() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn outer() -> Task[Wrap] {
    return worker()
}

async fn main() -> Wrap {
    let next = await outer()
    return await next
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected nested zero-sized task-handle async results to support chained await, got {diagnostics:?}"
    );
}

#[test]
fn accepts_awaiting_tuple_task_handle_aggregate_async_results() {
    let diagnostics = diagnostic_messages(
        r#"
async fn left() -> Int {
    return 1
}

async fn right() -> Int {
    return 2
}

async fn outer() -> (Task[Int], Task[Int]) {
    return (left(), right())
}

async fn main() -> Int {
    let pair = await outer()
    let first = await pair[0]
    let second = await pair[1]
    return first + second
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected tuple aggregate task-handle async results to support sibling awaits, got {diagnostics:?}"
    );
}

#[test]
fn accepts_awaiting_struct_task_handle_aggregate_async_results() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

struct Pending {
    first: Task[Wrap],
    second: Task[Wrap],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn outer() -> Pending {
    return Pending { first: worker(), second: worker() }
}

async fn main() -> Wrap {
    let pending = await outer()
    await pending.first
    return await pending.second
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected struct aggregate task-handle async results to support sibling awaits, got {diagnostics:?}"
    );
}

#[test]
fn accepts_awaiting_fixed_array_task_handle_aggregate_async_results() {
    let diagnostics = diagnostic_messages(
        r#"
async fn left() -> Int {
    return 1
}

async fn right() -> Int {
    return 2
}

async fn outer() -> [Task[Int]; 2] {
    return [left(), right()]
}

async fn main() -> Int {
    let tasks = await outer()
    let first = await tasks[0]
    let second = await tasks[1]
    return first + second
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected fixed-array aggregate task-handle async results to support sibling awaits, got {diagnostics:?}"
    );
}

#[test]
fn accepts_awaiting_nested_aggregate_task_handle_async_results() {
    let diagnostics = diagnostic_messages(
        r#"
struct Pending {
    task: Task[Int],
    value: Int,
}

async fn left() -> Int {
    return 1
}

async fn right() -> Int {
    return 2
}

async fn outer() -> [Pending; 2] {
    return [
        Pending { task: left(), value: 10 },
        Pending { task: right(), value: 20 },
    ]
}

async fn main() -> Int {
    let pending = await outer()
    let first = await pending[0].task
    let second = await pending[1].task
    return first + second + pending[0].value + pending[1].value
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected nested aggregate task-handle async results to support projection awaits, got {diagnostics:?}"
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
fn allows_spawning_bound_task_handle_helpers_inside_async_functions() {
    let diagnostics = diagnostic_messages(
        r#"
async fn worker() -> Int {
    return 1
}

fn schedule() -> Task[Int] {
    return worker()
}

async fn main() -> Int {
    let task = schedule()
    let running = spawn task
    return await running
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected bound task-handle helpers to be spawnable, got {diagnostics:?}"
    );
}

#[test]
fn allows_spawning_bound_zero_sized_task_handle_helpers_inside_async_functions() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn schedule() -> Task[Wrap] {
    return worker()
}

async fn main() -> Wrap {
    let task = schedule()
    let running = spawn task
    return await running
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected bound zero-sized task-handle helpers to be spawnable, got {diagnostics:?}"
    );
}

#[test]
fn allows_spawning_zero_sized_task_handle_helpers_inside_async_functions() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn schedule() -> Task[Wrap] {
    return worker()
}

async fn main() -> Wrap {
    let task = spawn schedule()
    return await task
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected zero-sized task-handle helper calls to be spawnable, got {diagnostics:?}"
    );
}

#[test]
fn accepts_returning_zero_sized_async_task_handles_through_explicit_task_types() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn schedule() -> Task[Wrap] {
    return worker()
}

async fn main() -> Wrap {
    return await schedule()
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected explicit zero-sized Task-return helper flow to succeed, got {diagnostics:?}"
    );
}

#[test]
fn accepts_returning_zero_sized_local_task_handles_through_explicit_task_types() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn schedule() -> Task[Wrap] {
    let task = worker()
    return task
}

async fn main() -> Wrap {
    return await schedule()
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected explicit zero-sized local-return Task helper flow to succeed, got {diagnostics:?}"
    );
}

#[test]
fn accepts_passing_zero_sized_async_calls_to_task_handle_parameters() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn main() -> Wrap {
    return await forward(worker())
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected zero-sized task-handle parameter flow to succeed, got {diagnostics:?}"
    );
}

#[test]
fn accepts_conditionally_returning_zero_sized_task_handles_from_helpers() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn choose(flag: Bool, first: Task[Wrap], second: Task[Wrap]) -> Task[Wrap] {
    if flag {
        return first
    }
    return second
}

async fn main(flag: Bool) -> Wrap {
    return await choose(flag, worker(), worker())
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected conditional zero-sized Task helper returns to type-check, got {diagnostics:?}"
    );
}

#[test]
fn accepts_reassigning_zero_sized_helper_consumed_task_handles() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main() -> Wrap {
    var task = worker()
    let forwarded = forward(task)
    task = worker()
    return await task
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected helper-consumed zero-sized task handle reassignment to type-check, got {diagnostics:?}"
    );
}

#[test]
fn accepts_reinitializing_zero_sized_task_handles_for_later_cleanup_reads() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn fresh_worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main() -> Int {
    var task = worker()
    defer await task
    defer {
        task = fresh_worker();
        ""
    }
    defer await task
    return 0
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected zero-sized task handle cleanup reinitialization to type-check, got {diagnostics:?}"
    );
}

#[test]
fn accepts_conditional_cleanup_over_zero_sized_task_handles() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn fresh_worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main(flag: Bool) -> Int {
    var task = worker()
    defer await task
    defer if flag { await task } else { await fresh_worker() }
    return 0
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected conditional cleanup over zero-sized task handles to type-check, got {diagnostics:?}"
    );
}

#[test]
fn accepts_conditional_cleanup_over_zero_sized_helper_consumes() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main(flag: Bool) -> Int {
    var task = worker()
    defer await task
    defer if flag { forward(task) } else { forward(worker()) }
    return 0
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected conditional zero-sized helper cleanup to type-check, got {diagnostics:?}"
    );
}

#[test]
fn accepts_conditional_cleanup_reinitializing_zero_sized_task_handles() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn fresh_worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main(flag: Bool) -> Int {
    var task = worker()
    defer if flag { task = fresh_worker(); "" } else { "" }
    defer await task
    return 0
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected conditional zero-sized task cleanup reinitialization to type-check, got {diagnostics:?}"
    );
}

#[test]
fn accepts_conditional_cleanup_reinitializing_and_consuming_zero_sized_task_handles() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn fresh_worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main(flag: Bool) -> Int {
    var task = worker()
    defer await task
    defer if flag { task = fresh_worker(); task } else { forward(task) }
    return 0
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected conditional zero-sized task cleanup reinit/consume to type-check, got {diagnostics:?}"
    );
}

#[test]
fn accepts_conditional_cleanup_reinitializing_and_helper_consuming_zero_sized_task_handles() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn fresh_worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main(flag: Bool) -> Int {
    var task = worker()
    defer await task
    defer if flag { task = fresh_worker(); forward(task) } else { forward(task) }
    return 0
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected conditional zero-sized task cleanup reinit/helper-consume to type-check, got {diagnostics:?}"
    );
}

#[test]
fn accepts_conditional_cleanup_helper_consuming_and_reinitializing_zero_sized_task_handles() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn fresh_worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main(flag: Bool) -> Int {
    var task = worker()
    defer await task
    defer if flag { forward(task) } else { task = fresh_worker(); forward(task) }
    return 0
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected conditional zero-sized task cleanup helper-consume/reinit to type-check, got {diagnostics:?}"
    );
}

#[test]
fn accepts_branch_joining_zero_sized_task_handles_before_await() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main(flag: Bool) -> Wrap {
    let task = worker()
    if flag {
        await task
    } else {
        Wrap { values: [] }
    }
    return await task
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected zero-sized task handle branch join to type-check, got {diagnostics:?}"
    );
}

#[test]
fn accepts_branch_joining_and_helper_reinitializing_zero_sized_task_handles_before_await() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn fresh_worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main(flag: Bool) -> Wrap {
    var task = worker()
    if flag {
        forward(task)
    } else {
        task = fresh_worker()
    }
    return await task
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected branch-joined zero-sized task helper consume/reinit flow to type-check, got {diagnostics:?}"
    );
}

#[test]
fn accepts_branch_joining_and_reinitializing_spawned_zero_sized_task_handles_before_await() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn fresh_worker() -> Wrap {
    return Wrap { values: [] }
}

    async fn main(flag: Bool) -> Wrap {
        var task = worker()
        if flag {
            let running = spawn task;
            task = fresh_worker();
            await running
        } else {
            task = fresh_worker()
        }
        return await task
    }
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected branch-joined zero-sized spawned task reinit flow to type-check, got {diagnostics:?}"
    );
}

#[test]
fn accepts_reverse_branch_joining_and_reinitializing_spawned_zero_sized_task_handles_before_await()
{
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn fresh_worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main(flag: Bool) -> Wrap {
    var task = worker()
    if flag {
        task = fresh_worker()
    } else {
        let running = spawn task;
        task = fresh_worker();
        await running
    }
    return await task
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected reverse-branch zero-sized spawned task reinit flow to type-check, got {diagnostics:?}"
    );
}

#[test]
fn accepts_conditionally_returning_spawned_zero_sized_task_handles_from_helpers() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn choose(flag: Bool, task: Task[Wrap]) -> Wrap {
    if flag {
        let running = spawn task;
        return await running
    }
    return await task
}

async fn main(flag: Bool) -> Wrap {
    return await choose(flag, worker())
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected helper conditional spawned task return flow to type-check, got {diagnostics:?}"
    );
}

#[test]
fn accepts_reverse_branch_conditionally_returning_spawned_zero_sized_task_handles_from_helpers() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn choose(flag: Bool, task: Task[Wrap]) -> Wrap {
    if flag {
        return await task
    }
    let running = spawn task;
    return await running
}

async fn main(flag: Bool) -> Wrap {
    return await choose(flag, worker())
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected reverse-branch helper conditional spawned task return flow to type-check, got {diagnostics:?}"
    );
}

#[test]
fn accepts_conditionally_returning_spawned_zero_sized_task_handles_from_async_calls() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn choose(flag: Bool) -> Wrap {
    if flag {
        let running = spawn worker();
        return await running
    }
    return await worker()
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected conditional spawned async-call flow to type-check, got {diagnostics:?}"
    );
}

#[test]
fn accepts_awaiting_projected_zero_sized_task_handles_from_tuples() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main() -> Wrap {
    let pair = (worker(), worker())
    return await pair[0]
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected projected zero-sized task-handle await flow to type-check, got {diagnostics:?}"
    );
}

#[test]
fn accepts_awaiting_projected_zero_sized_task_handles_from_fixed_arrays() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main() -> Wrap {
    let pair = [worker(), worker()]
    return await pair[0]
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected fixed-array projected zero-sized task-handle await flow to type-check, got {diagnostics:?}"
    );
}

#[test]
fn accepts_reinitializing_projected_zero_sized_task_handles_from_fixed_arrays() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main() -> Wrap {
    var tasks = [worker(), worker()]
    let first = await tasks[0]
    tasks[0] = worker()
    return await tasks[0]
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected fixed-array projected task-handle reinitialization to type-check, got {diagnostics:?}"
    );
}

#[test]
fn accepts_conditionally_reinitializing_projected_zero_sized_task_handles_from_fixed_arrays() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main(flag: Bool) -> Wrap {
    var tasks = [worker(), worker()]
    if flag {
        let first = await tasks[0]
        tasks[0] = worker()
    }
    return await tasks[0]
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected conditional fixed-array projected task-handle reinitialization to type-check, got {diagnostics:?}"
    );
}

#[test]
fn accepts_dynamic_task_handle_array_index_assignment_targets() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main(index: Int) -> Wrap {
    var tasks = [worker(), worker()]
    tasks[index] = worker()
    return await tasks[0]
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected dynamic task-handle array assignment to type-check, got {diagnostics:?}"
    );
}

#[test]
fn accepts_spawning_projected_zero_sized_task_handles_from_tuples() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main() -> Wrap {
    let pair = (worker(), worker())
    let running = spawn pair[0]
    return await running
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected projected zero-sized task-handle spawn flow to type-check, got {diagnostics:?}"
    );
}

#[test]
fn accepts_spawning_projected_zero_sized_task_handles_from_fixed_arrays() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main() -> Wrap {
    let pair = [worker(), worker()]
    let running = spawn pair[0]
    return await running
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected fixed-array projected zero-sized task-handle spawn flow to type-check, got {diagnostics:?}"
    );
}

#[test]
fn accepts_awaiting_projected_zero_sized_task_handles_from_struct_fields() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

struct TaskPair {
    task: Task[Wrap],
    value: Int,
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main() -> Wrap {
    let pair = TaskPair { task: worker(), value: 1 }
    return await pair.task
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected projected zero-sized struct-field task-handle await flow to type-check, got {diagnostics:?}"
    );
}

#[test]
fn accepts_spawning_projected_zero_sized_task_handles_from_struct_fields() {
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

struct TaskPair {
    task: Task[Wrap],
    value: Int,
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn main() -> Wrap {
    let pair = TaskPair { task: worker(), value: 1 }
    let running = spawn pair.task
    return await running
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected projected zero-sized struct-field task-handle spawn flow to type-check, got {diagnostics:?}"
    );
}

#[test]
fn accepts_reverse_branch_conditionally_returning_spawned_zero_sized_task_handles_from_async_calls()
{
    let diagnostics = diagnostic_messages(
        r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn choose(flag: Bool) -> Wrap {
    if flag {
        return await worker()
    }
    let running = spawn worker();
    return await running
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected reverse-branch conditional spawned async-call flow to type-check, got {diagnostics:?}"
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
fn binds_for_await_pattern_to_task_array_elements() {
    let diagnostics = diagnostic_messages(
        r#"
fn make_handle(value: Int) -> Task[Int] {
    return worker(value)
}

async fn worker(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    var total = 0
    for await task in [make_handle(1), make_handle(2)] {
        total = total + await task
    }
    return total
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected for-await task-array element binding to succeed, got {diagnostics:?}"
    );
}

#[test]
fn reports_precise_for_await_task_array_element_types() {
    let diagnostics = diagnostic_messages(
        r#"
fn make_handle(value: Int) -> Task[Int] {
    return worker(value)
}

async fn worker(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    var total = 0
    for await task in [make_handle(1), make_handle(2)] {
        total = total + task
    }
    return total
}
"#,
    );

    assert!(
        diagnostics.contains(
            &"binary operator `+` expects numeric operands, found `Int` and `Task[Int]`"
                .to_string()
        ),
        "expected precise task-element diagnostic inside for-await, got {diagnostics:?}"
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
