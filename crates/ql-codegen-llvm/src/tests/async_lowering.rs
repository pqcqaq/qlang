use super::*;

#[test]
fn rejects_unsupported_for_lowering() {
    let messages = emit_error(
        r#"
fn main() -> Int {
for value in 0 {
    break
}
return 0
}
"#,
    );

    assert!(messages.iter().any(|message| {
        message == "LLVM IR backend foundation does not support `for` lowering yet"
    }));
    assert!(messages.iter().all(|message| {
        !message.contains("could not resolve LLVM type for local")
            && !message.contains("could not infer LLVM type for MIR local")
    }));
}

#[test]
fn emits_await_lowering_for_scalar_async_results() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
async fn worker() -> Int {
return 1
}

async fn helper() -> Int {
return await worker()
}
"#,
        CodegenMode::Library,
        &runtime_hooks,
    );

    assert!(rendered.contains("declare ptr @qlrt_task_await(ptr)"));
    assert!(rendered.contains("declare void @qlrt_task_result_release(ptr)"));
    assert!(rendered.contains("define i64 @ql_1_helper__async_body(ptr %frame)"));
    assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
    assert!(rendered.contains("load i64, ptr %t"));
    assert!(rendered.contains("call void @qlrt_task_result_release(ptr %t"));
}

#[test]
fn emits_await_lowering_for_bound_direct_async_handles() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
async fn worker() -> Int {
return 1
}

async fn helper() -> Int {
let task = worker()
return await task
}
"#,
        CodegenMode::Library,
        &runtime_hooks,
    );

    assert!(rendered.contains("call ptr @ql_0_worker()"));
    assert!(rendered.contains("store ptr %t"));
    assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
    assert!(rendered.contains("load i64, ptr %t"));
}

#[test]
fn emits_await_lowering_for_task_handle_helpers() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
async fn worker() -> Int {
return 1
}

fn schedule() -> Task[Int] {
return worker()
}

async fn helper() -> Int {
return await schedule()
}
"#,
        CodegenMode::Library,
        &runtime_hooks,
    );

    assert!(rendered.contains("define ptr @ql_1_schedule()"));
    assert!(rendered.contains("call ptr @ql_0_worker()"));
    assert!(rendered.contains("call ptr @ql_1_schedule()"));
    assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
    assert!(rendered.contains("load i64, ptr %t"));
}

#[test]
fn emits_chained_await_lowering_for_nested_task_handle_async_results() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
async fn worker() -> Int {
return 1
}

async fn outer() -> Task[Int] {
return worker()
}

async fn helper() -> Int {
let next = await outer()
return await next
}
"#,
        CodegenMode::Library,
        &runtime_hooks,
    );

    assert!(rendered.contains("call ptr @ql_0_worker()"));
    assert!(rendered.contains("call ptr @ql_1_outer()"));
    assert!(rendered.matches("@qlrt_task_await").count() >= 2);
    assert!(rendered.contains("load ptr, ptr %t"));
    assert!(rendered.contains("load i64, ptr %t"));
}

#[test]
fn emits_chained_await_lowering_for_tuple_task_handle_aggregate_async_results() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
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

async fn helper() -> Int {
let pair = await outer()
let first = await pair[0]
let second = await pair[1]
return first + second
}
"#,
        CodegenMode::Library,
        &runtime_hooks,
    );

    assert!(rendered.contains("call ptr @ql_2_outer()"));
    assert!(rendered.matches("@qlrt_task_await").count() >= 3);
    assert!(rendered.contains("load { ptr, ptr }, ptr %t"));
    assert!(rendered.contains("getelementptr inbounds { ptr, ptr }, ptr"));
    assert!(rendered.matches("load ptr, ptr").count() >= 2);
}

#[test]
fn emits_chained_await_lowering_for_struct_task_handle_aggregate_async_results() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
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

async fn helper() -> Wrap {
let pending = await outer()
await pending.first
return await pending.second
}
"#,
        CodegenMode::Library,
        &runtime_hooks,
    );

    assert!(rendered.matches("@qlrt_task_await").count() >= 3);
    assert!(rendered.contains("load { ptr, ptr }, ptr %t"));
    assert!(rendered.contains("getelementptr inbounds { ptr, ptr }, ptr"));
    assert!(rendered.contains("load { [0 x i64] }, ptr"));
}

#[test]
fn emits_chained_await_lowering_for_array_task_handle_aggregate_async_results() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
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

async fn helper() -> Int {
let tasks = await outer()
let first = await tasks[0]
let second = await tasks[1]
return first + second
}
"#,
        CodegenMode::Library,
        &runtime_hooks,
    );

    assert!(rendered.contains("call ptr @ql_2_outer()"));
    assert!(rendered.matches("@qlrt_task_await").count() >= 3);
    assert!(rendered.contains("load [2 x ptr], ptr %t"));
    assert!(rendered.contains("getelementptr inbounds [2 x ptr], ptr"));
    assert!(rendered.matches("load ptr, ptr").count() >= 2);
}

#[test]
fn emits_chained_await_lowering_for_nested_aggregate_task_handle_async_results() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
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

async fn helper() -> Int {
let pending = await outer()
let first = await pending[0].task
let second = await pending[1].task
return first + second + pending[0].value + pending[1].value
}
"#,
        CodegenMode::Library,
        &runtime_hooks,
    );

    assert!(rendered.matches("@qlrt_task_await").count() >= 3);
    assert!(rendered.contains("load [2 x { ptr, i64 }], ptr %t"));
    assert!(rendered.contains("getelementptr inbounds [2 x { ptr, i64 }], ptr"));
    assert!(rendered.contains("getelementptr inbounds { ptr, i64 }, ptr"));
    assert!(rendered.matches("load ptr, ptr").count() >= 2);
}

#[test]
fn emits_await_lowering_for_bound_task_handle_helpers() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
async fn worker() -> Int {
return 1
}

fn schedule() -> Task[Int] {
return worker()
}

async fn helper() -> Int {
let task = schedule()
return await task
}
"#,
        CodegenMode::Library,
        &runtime_hooks,
    );

    assert!(rendered.contains("define ptr @ql_1_schedule()"));
    assert!(rendered.contains("call ptr @ql_1_schedule()"));
    assert!(rendered.contains("store ptr %t"));
    assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
    assert!(rendered.contains("load i64, ptr %t"));
}

#[test]
fn emits_await_lowering_for_local_returned_task_handle_helpers() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
async fn worker() -> Int {
return 1
}

fn schedule() -> Task[Int] {
let task = worker()
return task
}

async fn helper() -> Int {
return await schedule()
}
"#,
        CodegenMode::Library,
        &runtime_hooks,
    );

    assert!(rendered.contains("define ptr @ql_1_schedule()"));
    assert!(rendered.contains("call ptr @ql_0_worker()"));
    assert!(rendered.contains("store ptr %t"));
    assert!(rendered.contains("call ptr @ql_1_schedule()"));
    assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
    assert!(rendered.contains("load i64, ptr %t"));
}

#[test]
fn emits_await_lowering_for_zero_sized_recursive_aggregate_task_handle_helpers() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
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

async fn helper() -> Wrap {
return await schedule()
}
"#,
        CodegenMode::Library,
        &runtime_hooks,
    );

    assert!(rendered.contains("define ptr @ql_2_schedule()"));
    assert!(rendered.contains("call ptr @ql_1_worker()"));
    assert!(rendered.contains("call ptr @ql_2_schedule()"));
    assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
    assert!(rendered.contains("load { [0 x i64] }, ptr %t"));
}

#[test]
fn emits_await_lowering_for_bound_zero_sized_recursive_aggregate_task_handle_helpers() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
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

async fn helper() -> Wrap {
let task = schedule()
return await task
}
"#,
        CodegenMode::Library,
        &runtime_hooks,
    );

    assert!(rendered.contains("define ptr @ql_2_schedule()"));
    assert!(rendered.contains("call ptr @ql_2_schedule()"));
    assert!(rendered.contains("store ptr %t"));
    assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
    assert!(rendered.contains("load { [0 x i64] }, ptr %t"));
}

#[test]
fn emits_await_lowering_for_local_returned_zero_sized_recursive_aggregate_task_handles() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
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

async fn helper() -> Wrap {
return await schedule()
}
"#,
        CodegenMode::Library,
        &runtime_hooks,
    );

    assert!(rendered.contains("define ptr @ql_2_schedule()"));
    assert!(rendered.contains("call ptr @ql_1_worker()"));
    assert!(rendered.contains("store ptr %t"));
    assert!(rendered.contains("call ptr @ql_2_schedule()"));
    assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
    assert!(rendered.contains("load { [0 x i64] }, ptr %t"));
}

#[test]
fn emits_await_lowering_for_forwarded_task_handle_arguments() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
async fn worker() -> Int {
return 1
}

fn forward(task: Task[Int]) -> Task[Int] {
return task
}

async fn helper() -> Int {
let task = worker()
let forwarded = forward(task)
return await forwarded
}
"#,
        CodegenMode::Library,
        &runtime_hooks,
    );

    assert!(rendered.contains("define ptr @ql_1_forward(ptr %arg0)"));
    assert!(rendered.matches("_forward(").count() >= 2);
    assert!(rendered.contains("call ptr @ql_0_worker()"));
    assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
    assert!(rendered.contains("load i64, ptr %t"));
}

#[test]
fn emits_await_lowering_for_forwarded_zero_sized_recursive_aggregate_task_handles() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
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

async fn helper() -> Wrap {
let task = worker()
let forwarded = forward(task)
return await forwarded
}
"#,
        CodegenMode::Library,
        &runtime_hooks,
    );

    assert!(rendered.contains("define ptr @ql_2_forward(ptr %arg0)"));
    assert!(rendered.matches("_forward(").count() >= 2);
    assert!(rendered.contains("call ptr @ql_1_worker()"));
    assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
    assert!(rendered.contains("load { [0 x i64] }, ptr %t"));
}

#[test]
fn emits_await_lowering_for_void_async_results() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
async fn worker() -> Void {
return
}

async fn helper() -> Void {
await worker()
return
}
"#,
        CodegenMode::Library,
        &runtime_hooks,
    );

    assert!(rendered.contains("define void @ql_1_helper__async_body(ptr %frame)"));
    assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
    assert!(rendered.contains("call void @qlrt_task_result_release(ptr %t"));
    assert!(!rendered.contains("load void"));
}

#[test]
fn emits_await_lowering_for_tuple_async_results() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
async fn worker() -> (Bool, Int) {
return (true, 1)
}

async fn helper() -> (Bool, Int) {
return await worker()
}
"#,
        CodegenMode::Library,
        &runtime_hooks,
    );

    assert!(rendered.contains("define { i1, i64 } @ql_1_helper__async_body(ptr %frame)"));
    assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
    assert!(rendered.contains("load { i1, i64 }, ptr %t"));
    assert!(rendered.contains("call void @qlrt_task_result_release(ptr %t"));
}

#[test]
fn rejects_await_lowering_without_task_result_release_hook() {
    let messages = emit_error_with_runtime_hooks(
        r#"
async fn worker() -> Int {
return 1
}

async fn helper() -> Int {
return await worker()
}
"#,
        CodegenMode::Library,
        &[
            runtime_hook_signature(RuntimeHook::AsyncTaskCreate),
            runtime_hook_signature(RuntimeHook::TaskAwait),
        ],
    );

    assert!(messages.iter().any(|message| {
        message
            == "LLVM IR backend foundation requires the `task-result-release` runtime hook before lowering `await` expressions"
    }));
}

#[test]
fn emits_fire_and_forget_spawn_lowering_in_async_library_body() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
async fn worker() -> Int {
return 1
}

async fn helper() -> Int {
spawn worker()
return 0
}
"#,
        CodegenMode::Library,
        &runtime_hooks,
    );

    assert!(rendered.contains("declare ptr @qlrt_executor_spawn(ptr, ptr)"));
    assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %t"));
}

#[test]
fn emits_spawn_handle_lowering_and_awaits_spawned_task_in_async_library_body() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
async fn worker() -> Int {
return 1
}

async fn helper() -> Int {
let task = spawn worker()
return await task
}
"#,
        CodegenMode::Library,
        &runtime_hooks,
    );

    assert!(rendered.contains("declare ptr @qlrt_executor_spawn(ptr, ptr)"));
    assert!(rendered.contains("declare ptr @qlrt_task_await(ptr)"));
    assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %t"));
    assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
}

#[test]
fn emits_spawn_lowering_for_bound_task_handles() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
async fn worker() -> Int {
return 1
}

async fn helper() -> Int {
let task = worker()
let running = spawn task
return await running
}
"#,
        CodegenMode::Library,
        &runtime_hooks,
    );

    assert!(rendered.contains("call ptr @ql_0_worker()"));
    assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %t"));
    assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
}

#[test]
fn emits_spawn_lowering_for_bound_zero_sized_recursive_aggregate_task_handles() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
struct Wrap {
values: [Int; 0],
}

async fn worker() -> Wrap {
return Wrap { values: [] }
}

async fn helper() -> Wrap {
let task = worker()
let running = spawn task
return await running
}
"#,
        CodegenMode::Library,
        &runtime_hooks,
    );

    assert!(rendered.contains("call ptr @ql_1_worker()"));
    assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %t"));
    assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
    assert!(rendered.contains("load { [0 x i64] }, ptr"));
}

#[test]
fn emits_spawn_lowering_for_task_handle_helpers() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
async fn worker() -> Int {
return 1
}

fn schedule() -> Task[Int] {
return worker()
}

async fn helper() -> Int {
let task = spawn schedule()
return await task
}
"#,
        CodegenMode::Library,
        &runtime_hooks,
    );

    assert!(rendered.matches("_schedule(").count() >= 2);
    assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %t"));
    assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
}

#[test]
fn emits_spawn_lowering_for_bound_task_handle_helpers() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
async fn worker() -> Int {
return 1
}

fn schedule() -> Task[Int] {
return worker()
}

async fn helper() -> Int {
let task = schedule()
let running = spawn task
return await running
}
"#,
        CodegenMode::Library,
        &runtime_hooks,
    );

    assert!(rendered.contains("define ptr @ql_1_schedule()"));
    assert!(rendered.contains("call ptr @ql_1_schedule()"));
    assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %t"));
    assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
}

#[test]
fn emits_spawn_lowering_for_zero_sized_recursive_aggregate_task_handle_helpers() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
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

async fn helper() -> Wrap {
let task = spawn schedule()
return await task
}
"#,
        CodegenMode::Library,
        &runtime_hooks,
    );

    assert!(rendered.matches("_schedule(").count() >= 2);
    assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %t"));
    assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
    assert!(rendered.contains("load { [0 x i64] }, ptr"));
}

#[test]
fn emits_spawn_lowering_for_conditional_zero_sized_recursive_aggregate_task_handle_helpers() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
struct Wrap {
values: [Int; 0],
}

async fn worker() -> Wrap {
return Wrap { values: [] }
}

async fn choose(flag: Bool, task: Task[Wrap]) -> Wrap {
if flag {
    let running = spawn task
    return await running
}
return await task
}

async fn helper(flag: Bool) -> Wrap {
return await choose(flag, worker())
}
"#,
        CodegenMode::Library,
        &runtime_hooks,
    );

    assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %t"));
    assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
    assert!(rendered.contains("load { [0 x i64] }, ptr"));
}

#[test]
fn emits_spawn_lowering_for_reverse_branch_conditional_zero_sized_recursive_aggregate_task_handle_helpers()
 {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
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
let running = spawn task
return await running
}

async fn helper(flag: Bool) -> Wrap {
return await choose(flag, worker())
}
"#,
        CodegenMode::Library,
        &runtime_hooks,
    );

    assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %t"));
    assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
    assert!(rendered.contains("load { [0 x i64] }, ptr"));
}

#[test]
fn emits_spawn_lowering_for_conditional_zero_sized_recursive_aggregate_async_calls() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
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
        CodegenMode::Library,
        &runtime_hooks,
    );

    assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %t"));
    assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
    assert!(rendered.contains("load { [0 x i64] }, ptr"));
}

#[test]
fn emits_spawn_lowering_for_reverse_branch_conditional_zero_sized_recursive_aggregate_async_calls()
{
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
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
        CodegenMode::Library,
        &runtime_hooks,
    );

    assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %t"));
    assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
    assert!(rendered.contains("load { [0 x i64] }, ptr"));
}

#[test]
fn emits_spawn_lowering_for_branch_join_reinitializing_zero_sized_recursive_aggregate_async_calls()
{
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
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

async fn helper(flag: Bool) -> Wrap {
var task = worker()
if flag {
    let running = spawn task
    task = fresh_worker()
    return await running
} else {
    task = fresh_worker()
}
return await task
}
"#,
        CodegenMode::Library,
        &runtime_hooks,
    );

    assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %t"));
    assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
    assert!(rendered.contains("load { [0 x i64] }, ptr"));
}

#[test]
fn emits_spawn_lowering_for_reverse_branch_join_reinitializing_zero_sized_recursive_aggregate_async_calls()
 {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
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

async fn helper(flag: Bool) -> Wrap {
var task = worker()
if flag {
    task = fresh_worker()
} else {
    let running = spawn task
    task = fresh_worker()
    return await running
}
return await task
}
"#,
        CodegenMode::Library,
        &runtime_hooks,
    );

    assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %t"));
    assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
    assert!(rendered.contains("load { [0 x i64] }, ptr"));
}

#[test]
fn emits_spawn_lowering_for_bound_zero_sized_recursive_aggregate_task_handle_helpers() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
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

async fn helper() -> Wrap {
let task = schedule()
let running = spawn task
return await running
}
"#,
        CodegenMode::Library,
        &runtime_hooks,
    );

    assert!(rendered.contains("define ptr @ql_2_schedule()"));
    assert!(rendered.contains("call ptr @ql_2_schedule()"));
    assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %t"));
    assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
    assert!(rendered.contains("load { [0 x i64] }, ptr"));
}

#[test]
fn emits_spawn_lowering_for_forwarded_task_handle_arguments() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
async fn worker() -> Int {
return 1
}

fn forward(task: Task[Int]) -> Task[Int] {
return task
}

async fn helper() -> Int {
let task = worker()
let running = spawn forward(task)
return await running
}
"#,
        CodegenMode::Library,
        &runtime_hooks,
    );

    assert!(rendered.contains("define ptr @ql_1_forward(ptr %arg0)"));
    assert!(rendered.matches("_forward(").count() >= 2);
    assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %t"));
    assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
}

#[test]
fn emits_spawn_lowering_for_forwarded_zero_sized_recursive_aggregate_task_handles() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
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

async fn helper() -> Wrap {
let task = worker()
let running = spawn forward(task)
return await running
}
"#,
        CodegenMode::Library,
        &runtime_hooks,
    );

    assert!(rendered.contains("define ptr @ql_2_forward(ptr %arg0)"));
    assert!(rendered.matches("_forward(").count() >= 2);
    assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %t"));
    assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
    assert!(rendered.contains("load { [0 x i64] }, ptr"));
}

#[test]
fn emits_await_lowering_for_projected_zero_sized_task_handle_tuple_elements() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
struct Wrap {
values: [Int; 0],
}

async fn worker() -> Wrap {
return Wrap { values: [] }
}

async fn helper() -> Wrap {
let pair = (worker(), worker())
return await pair[0]
}
"#,
        CodegenMode::Library,
        &runtime_hooks,
    );

    assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
    assert!(rendered.contains("load { [0 x i64] }, ptr"));
    assert!(!rendered.contains("does not support field or index projections yet"));
}

#[test]
fn emits_await_lowering_for_projected_zero_sized_task_handle_array_elements() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
struct Wrap {
values: [Int; 0],
}

async fn worker() -> Wrap {
return Wrap { values: [] }
}

async fn helper() -> Wrap {
let pair = [worker(), worker()]
return await pair[0]
}
"#,
        CodegenMode::Library,
        &runtime_hooks,
    );

    assert!(rendered.contains("getelementptr inbounds [2 x ptr], ptr"));
    assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
    assert!(rendered.contains("load { [0 x i64] }, ptr"));
    assert!(!rendered.contains("does not support field or index projections yet"));
}

#[test]
fn emits_spawn_lowering_for_projected_zero_sized_task_handle_tuple_elements() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
struct Wrap {
values: [Int; 0],
}

async fn worker() -> Wrap {
return Wrap { values: [] }
}

async fn helper() -> Wrap {
let pair = (worker(), worker())
let running = spawn pair[0]
return await running
}
"#,
        CodegenMode::Library,
        &runtime_hooks,
    );

    assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %t"));
    assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
    assert!(rendered.contains("load { [0 x i64] }, ptr"));
    assert!(!rendered.contains("does not support field or index projections yet"));
}

#[test]
fn emits_spawn_lowering_for_projected_zero_sized_task_handle_array_elements() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
struct Wrap {
values: [Int; 0],
}

async fn worker() -> Wrap {
return Wrap { values: [] }
}

async fn helper() -> Wrap {
let pair = [worker(), worker()]
let running = spawn pair[0]
return await running
}
"#,
        CodegenMode::Library,
        &runtime_hooks,
    );

    assert!(rendered.contains("getelementptr inbounds [2 x ptr], ptr"));
    assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %t"));
    assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
    assert!(rendered.contains("load { [0 x i64] }, ptr"));
    assert!(!rendered.contains("does not support field or index projections yet"));
}

#[test]
fn emits_await_lowering_for_projected_zero_sized_task_handle_struct_fields() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
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

async fn helper() -> Wrap {
let pair = TaskPair { task: worker(), value: 1 }
return await pair.task
}
"#,
        CodegenMode::Library,
        &runtime_hooks,
    );

    assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
    assert!(rendered.contains("load { [0 x i64] }, ptr"));
    assert!(!rendered.contains("does not support field or index projections yet"));
}

#[test]
fn emits_spawn_lowering_for_projected_zero_sized_task_handle_struct_fields() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
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

async fn helper() -> Wrap {
let pair = TaskPair { task: worker(), value: 1 }
let running = spawn pair.task
return await running
}
"#,
        CodegenMode::Library,
        &runtime_hooks,
    );

    assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %t"));
    assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
    assert!(rendered.contains("load { [0 x i64] }, ptr"));
    assert!(!rendered.contains("does not support field or index projections yet"));
}

#[test]
fn emits_spawn_handle_lowering_for_zero_sized_recursive_aggregate_results() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
struct Wrap {
values: [Int; 0],
}

async fn worker() -> Wrap {
return Wrap { values: [] }
}

async fn helper() -> Wrap {
let task = spawn worker()
return await task
}
"#,
        CodegenMode::Library,
        &runtime_hooks,
    );

    assert!(rendered.contains("define { [0 x i64] } @ql_1_worker__async_body(ptr %frame)"));
    assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %t"));
    assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
    assert!(rendered.contains("load { [0 x i64] }, ptr"));
    assert!(rendered.contains("call void @qlrt_task_result_release(ptr"));
}

#[test]
fn emits_for_await_lowering_for_fixed_array_async_library_bodies() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::AsyncIteration,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
async fn helper() -> Int {
var total = 0
for await value in [1, 2, 3] {
    total = total + value
}
return total
}
"#,
        CodegenMode::Library,
        &runtime_hooks,
    );

    assert!(rendered.contains("declare ptr @qlrt_async_iter_next(ptr)"));
    assert!(rendered.contains("store i64 -1, ptr %for_await_index_bb"));
    assert!(rendered.contains("icmp ult i64"));
    assert!(rendered.contains("for_await_setup"));
    assert!(rendered.contains("getelementptr inbounds [3 x i64], ptr"));
    assert!(!rendered.contains("does not support `for await` lowering yet"));
}

#[test]
fn emits_for_await_lowering_for_homogeneous_tuple_async_library_bodies() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::AsyncIteration,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
async fn helper() -> Int {
var total = 0
for await value in (1, 2, 3) {
    total = total + value
}
return total
}
"#,
        CodegenMode::Library,
        &runtime_hooks,
    );

    assert!(rendered.contains("declare ptr @qlrt_async_iter_next(ptr)"));
    assert!(rendered.contains("store i64 -1, ptr %for_await_index_bb"));
    assert!(rendered.contains("icmp ult i64"));
    assert!(rendered.contains("for_await_setup"));
    assert!(rendered.contains("getelementptr inbounds { i64, i64, i64 }, ptr"));
    assert!(!rendered.contains("does not support `for await` lowering yet"));
}

#[test]
fn emits_for_lowering_for_fixed_array_program_bodies_without_async_runtime_hooks() {
    let rendered = emit_with_mode(
        r#"
fn main() -> Int {
var total = 0
for value in [1, 2, 3] {
    total = total + value
}
return total
}
"#,
        CodegenMode::Program,
    );

    assert!(!rendered.contains("@qlrt_async_iter_next"));
    assert!(rendered.contains("store i64 -1, ptr %for_await_index_bb"));
    assert!(rendered.contains("icmp ult i64"));
    assert!(rendered.contains("for_await_setup"));
    assert!(rendered.contains("getelementptr inbounds [3 x i64], ptr"));
    assert!(!rendered.contains("does not support `for` lowering yet"));
}

#[test]
fn emits_for_lowering_for_homogeneous_tuple_program_bodies_without_async_runtime_hooks() {
    let rendered = emit_with_mode(
        r#"
fn main() -> Int {
var total = 0
for value in (1, 2, 3) {
    total = total + value
}
return total
}
"#,
        CodegenMode::Program,
    );

    assert!(!rendered.contains("@qlrt_async_iter_next"));
    assert!(rendered.contains("store i64 -1, ptr %for_await_index_bb"));
    assert!(rendered.contains("icmp ult i64"));
    assert!(rendered.contains("for_await_setup"));
    assert!(rendered.contains("getelementptr inbounds { i64, i64, i64 }, ptr"));
    assert!(!rendered.contains("does not support `for` lowering yet"));
}

#[test]
fn emits_for_lowering_for_const_backed_tuple_program_bodies() {
    let rendered = emit_with_mode(
        r#"
const VALUES: (Int, Int, Int) = (1, 2, 3)

fn main() -> Int {
var total = 0
for value in VALUES {
    total = total + value
}
return total
}
"#,
        CodegenMode::Program,
    );

    assert!(rendered.contains("%for_iterable_bb"));
    assert!(rendered.contains("insertvalue { i64, i64, i64 }"));
    assert!(rendered.contains("getelementptr inbounds { i64, i64, i64 }, ptr %for_iterable_bb"));
    assert!(!rendered.contains("does not support `for` lowering yet"));
}

#[test]
fn emits_for_lowering_for_import_aliased_static_array_program_bodies() {
    let rendered = emit_with_mode(
        r#"
use VALUES as INPUT
static VALUES: [Int; 3] = [1, 2, 3]

fn main() -> Int {
var total = 0
for value in INPUT {
    total = total + value
}
return total
}
"#,
        CodegenMode::Program,
    );

    assert!(rendered.contains("%for_iterable_bb"));
    assert!(rendered.contains("insertvalue [3 x i64]"));
    assert!(rendered.contains("getelementptr inbounds [3 x i64], ptr %for_iterable_bb"));
    assert!(!rendered.contains("does not support `for` lowering yet"));
}

#[test]
fn emits_for_lowering_for_projected_control_flow_roots_in_program_bodies() {
    let rendered = emit_with_mode(
        r#"
struct Boxed {
values: [Int; 3],
}

fn main() -> Int {
let branch = true
var boxed = Boxed { values: [0, 0, 0] }
var total = 0
for value in ({ let current = Boxed { values: [1, 2, 3] }; current }).values {
    total = total + value
}
for value in (boxed = Boxed { values: [4, 5, 6] }).values {
    total = total + value
}
for value in (if branch { Boxed { values: [7, 8, 9] } } else { Boxed { values: [10, 11, 12] } }).values {
    total = total + value
}
for item in (match branch {
    true => Boxed { values: [13, 14, 15] },
    false => Boxed { values: [16, 17, 18] },
}).values {
    total = total + item
}
return total
}
"#,
        CodegenMode::Program,
    );

    assert!(
        rendered
            .matches("store i64 -1, ptr %for_await_index_bb")
            .count()
            >= 4
    );
    assert!(rendered.contains("for_await_setup"));
    assert!(rendered.contains("insertvalue { [3 x i64] }"));
    assert!(!rendered.contains("does not support `for` lowering yet"));
}

#[test]
fn emits_bind_pattern_destructuring_in_program_bodies() {
    let rendered = emit_with_mode(
        r#"
struct Pair {
left: Int,
right: Int,
}

fn pair_values() -> [Pair; 2] {
return [Pair { left: 20, right: 22 }, Pair { left: 24, right: 26 }]
}

fn main() -> Int {
let (first, second) = (1, 2)
var total = first + second
for (left, current) in ((4, 6), (8, 10)) {
    total = total + left + current
}
for Pair { left, right: current } in pair_values() {
    total = total + left + current
}
return total
}
"#,
        CodegenMode::Program,
    );

    assert!(rendered.matches("extractvalue { i64, i64 }").count() >= 6);
    assert!(rendered.contains("getelementptr inbounds [2 x { i64, i64 }], ptr"));
    assert!(!rendered.contains("only supports single-name binding patterns"));
    assert!(!rendered.contains("does not support `for` lowering yet"));
}

#[test]
fn inlines_runtime_support_for_async_dynamic_library_builds() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskAwait,
        RuntimeCapability::AsyncIteration,
    ]);
    let rendered = emit_with_runtime_hooks_and_inline_support(
        r#"
async fn seed_total() -> Int {
var total = 0
for await value in [20, 22] {
    total = total + value
}
return total
}

async fn helper() -> Int {
return await seed_total()
}

extern "c" pub fn q_add(left: Int, right: Int) -> Int {
return left + right
}
"#,
        CodegenMode::Library,
        &runtime_hooks,
        true,
    );

    assert!(!rendered.contains("declare ptr @qlrt_async_task_create(ptr, ptr)"));
    assert!(!rendered.contains("declare ptr @qlrt_task_await(ptr)"));
    assert!(!rendered.contains("declare void @qlrt_task_result_release(ptr)"));
    assert!(!rendered.contains("declare ptr @qlrt_async_iter_next(ptr)"));
    assert!(rendered.contains("define ptr @qlrt_async_task_create(ptr %entry_fn, ptr %frame)"));
    assert!(rendered.contains("define ptr @qlrt_task_await(ptr %handle)"));
    assert!(rendered.contains("define void @qlrt_task_result_release(ptr %result)"));
    assert!(rendered.contains("define ptr @qlrt_async_iter_next(ptr %iterator)"));
    assert!(rendered.contains("define i64 @q_add(i64 %arg0, i64 %arg1)"));
}

#[test]
fn rejects_unsupported_for_await_lowering_for_non_fixed_shape_iterables_without_iterable_noise() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::AsyncIteration,
    ]);
    let messages = emit_error_with_runtime_hooks(
        r#"
async fn helper() -> Int {
for await value in 0 {
    break
}
return 0
}
"#,
        CodegenMode::Library,
        &runtime_hooks,
    );

    assert!(messages.iter().any(|message| {
        message == "LLVM IR backend foundation does not support `for await` lowering yet"
    }));
    assert!(messages.iter().all(|message| {
        message != "LLVM IR backend foundation does not support `for` lowering yet"
            && message != "LLVM IR backend foundation does not support array values yet"
            && !message.contains("could not resolve LLVM type for local")
            && !message.contains("could not infer LLVM type for MIR local")
    }));
}
