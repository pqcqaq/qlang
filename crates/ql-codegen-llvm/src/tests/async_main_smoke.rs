use super::*;

// -----------------------------------------------------------------------
// async fn main — program-mode entry lifecycle tests (P7.4)
// -----------------------------------------------------------------------

#[test]
fn emits_async_main_entry_lifecycle_in_program_mode() {
    // async fn main drives the full task lifecycle from the C `@main` entry:
    //   task    = @ql_N_main()                        (task-create wrapper)
    //   join    = qlrt_executor_spawn(null, task)
    //   res_ptr = qlrt_task_await(join)
    //   ret_val = load i64, ptr res_ptr
    //             qlrt_task_result_release(res_ptr)
    //   exit    = trunc i64 ret_val to i32
    //             ret i32 exit
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

async fn main() -> Int {
return await worker()
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    // Runtime hooks must be defined in-program so native executable links succeed.
    assert!(rendered.contains("define ptr @qlrt_async_task_create(ptr %entry_fn, ptr %frame)"));
    assert!(rendered.contains("define ptr @qlrt_executor_spawn(ptr %executor, ptr %task)"));
    assert!(rendered.contains("define ptr @qlrt_task_await(ptr %handle)"));
    assert!(rendered.contains("define void @qlrt_task_result_release(ptr %result)"));

    // The async body and task-create wrapper must be emitted.
    assert!(
        rendered.contains("define ptr @ql_1_main__async_body(ptr %frame)")
            || rendered.contains("define ptr @ql_0_main__async_body(ptr %frame)")
            || rendered.contains("__async_body")
    );
    assert!(rendered.contains("__async_entry"));
    assert!(rendered.contains("define ptr @ql_") && rendered.contains("@main("));

    // The C entry point must drive the lifecycle.
    assert!(rendered.contains("define i32 @main()"));
    assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
    assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
    assert!(rendered.contains("load i64, ptr %async_main_res"));
    assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
    assert!(rendered.contains("trunc i64 %async_main_ret to i32"));
    assert!(rendered.contains("ret i32 %async_main_exit"));
}

#[test]
fn emits_async_void_main_entry_lifecycle_in_program_mode() {
    // async fn main returning Void: the host entry skips the load and returns 0.
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
async fn main() -> Void {
return
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.contains("define i32 @main()"));
    assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
    assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
    assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
    assert!(rendered.contains("ret i32 0"));
    // No result load for void.
    assert!(!rendered.contains("load i64, ptr %async_main_res"));
}

#[test]
fn emits_async_main_entry_lifecycle_with_fixed_array_for_await_in_program_mode() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::TaskAwait,
        RuntimeCapability::AsyncIteration,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
async fn main() -> Int {
var total = 0
for await value in [1, 2, 3] {
    total = total + value
}
return total
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.contains("define ptr @qlrt_executor_spawn(ptr %executor, ptr %task)"));
    assert!(rendered.contains("define ptr @qlrt_task_await(ptr %handle)"));
    assert!(rendered.contains("define void @qlrt_task_result_release(ptr %result)"));
    assert!(rendered.contains("define ptr @qlrt_async_iter_next(ptr %iterator)"));
    assert!(rendered.contains("define i32 @main()"));
    assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
    assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
    assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
    assert!(rendered.contains("store i64 -1, ptr %for_await_index_bb"));
    assert!(rendered.contains("icmp ult i64"));
    assert!(rendered.contains("for_await_setup"));
    assert!(rendered.contains("getelementptr inbounds [3 x i64], ptr"));
    assert!(!rendered.contains("does not support `for await` lowering yet"));
}

#[test]
fn auto_awaits_task_array_elements_inside_for_await_in_program_mode() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::TaskAwait,
        RuntimeCapability::AsyncIteration,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
async fn worker(value: Int) -> Int {
return value
}

async fn main() -> Int {
var total = 0
for await value in [worker(20), worker(22)] {
    total = total + value
}
return total
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.contains("define i32 @main()"));
    assert!(rendered.contains("getelementptr inbounds [2 x ptr], ptr"));
    assert!(rendered.matches("call ptr @qlrt_task_await").count() >= 2);
    assert!(
        rendered
            .matches("call void @qlrt_task_result_release")
            .count()
            >= 2
    );
    assert!(rendered.contains("store i64 %t"));
    assert!(!rendered.contains("does not support `for await` lowering yet"));
}

#[test]
fn auto_awaits_task_tuple_elements_inside_for_await_in_program_mode() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::TaskAwait,
        RuntimeCapability::AsyncIteration,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
async fn worker(value: Int) -> Int {
return value
}

async fn main() -> Int {
var total = 0
for await value in (worker(20), worker(22)) {
    total = total + value
}
return total
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.contains("define i32 @main()"));
    assert!(rendered.contains("getelementptr inbounds { ptr, ptr }, ptr"));
    assert!(rendered.matches("call ptr @qlrt_task_await").count() >= 2);
    assert!(
        rendered
            .matches("call void @qlrt_task_result_release")
            .count()
            >= 2
    );
    assert!(rendered.contains("store i64 %t"));
    assert!(!rendered.contains("does not support `for await` lowering yet"));
}

#[test]
fn emits_for_await_lowering_for_projected_control_flow_roots_in_program_mode() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::TaskAwait,
        RuntimeCapability::AsyncIteration,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
struct Wrapper {
tasks: [Task[Int]; 2],
}

async fn worker(value: Int) -> Int {
return value
}

async fn main() -> Int {
let branch = true
var wrapper = Wrapper { tasks: [worker(0), worker(0)] }
var total = 0
for await value in ({ let current = Wrapper { tasks: [worker(1), worker(2)] }; current }).tasks {
    total = total + value
}
for await value in (wrapper = Wrapper { tasks: [worker(3), worker(4)] }).tasks {
    total = total + value
}
for await value in (if branch { Wrapper { tasks: [worker(5), worker(6)] } } else { Wrapper { tasks: [worker(7), worker(8)] } }).tasks {
    total = total + value
}
for await item in (match branch {
    true => Wrapper { tasks: [worker(9), worker(10)] },
    false => Wrapper { tasks: [worker(11), worker(12)] },
}).tasks {
    total = total + item
}
return total
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.matches("call ptr @qlrt_task_await").count() >= 4);
    assert!(
        rendered
            .matches("call void @qlrt_task_result_release")
            .count()
            >= 4
    );
    assert!(
        rendered
            .matches("store i64 -1, ptr %for_await_index_bb")
            .count()
            >= 4
    );
    assert!(rendered.contains("for_await_setup"));
    assert!(!rendered.contains("does not support `for await` lowering yet"));
}

#[test]
fn emits_async_main_entry_lifecycle_with_nested_task_handle_payload_in_program_mode() {
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

async fn outer() -> Task[Int] {
return worker()
}

async fn main() -> Int {
let next = await outer()
return await next
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.contains("define i32 @main()"));
    assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
    assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
    assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
    assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
    assert!(rendered.contains("load ptr, ptr %t"));
    assert!(!rendered.contains("does not support `await` lowering yet"));
}

#[test]
fn emits_async_main_entry_lifecycle_with_tuple_task_handle_payload_in_program_mode() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
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

async fn main() -> Int {
let pair = await outer()
let first = await pair[0]
let second = await pair[1]
return first + second
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.contains("define i32 @main()"));
    assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
    assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
    assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
    assert!(rendered.matches("@qlrt_task_await").count() >= 4);
    assert!(rendered.contains("load { ptr, ptr }, ptr %t"));
    assert!(rendered.contains("getelementptr inbounds { ptr, ptr }, ptr"));
    assert!(rendered.matches("load ptr, ptr").count() >= 2);
}

#[test]
fn emits_async_main_entry_lifecycle_with_array_task_handle_payload_in_program_mode() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
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

async fn main() -> Int {
let tasks = await outer()
let first = await tasks[0]
let second = await tasks[1]
return first + second
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.contains("define i32 @main()"));
    assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
    assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
    assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
    assert!(rendered.matches("@qlrt_task_await").count() >= 4);
    assert!(rendered.contains("load [2 x ptr], ptr %t"));
    assert!(rendered.contains("getelementptr inbounds [2 x ptr], ptr"));
    assert!(rendered.matches("load ptr, ptr").count() >= 2);
}

#[test]
fn emits_async_main_entry_lifecycle_with_nested_aggregate_task_handle_payload_in_program_mode() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
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

async fn main() -> Int {
let pending = await outer()
let first = await pending[0].task
let second = await pending[1].task
return first + second + pending[0].value + pending[1].value
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.contains("define i32 @main()"));
    assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
    assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
    assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
    assert!(rendered.matches("@qlrt_task_await").count() >= 4);
    assert!(rendered.contains("load [2 x { ptr, i64 }], ptr %t"));
    assert!(rendered.contains("getelementptr inbounds [2 x { ptr, i64 }], ptr"));
    assert!(rendered.contains("getelementptr inbounds { ptr, i64 }, ptr"));
    assert!(rendered.matches("load ptr, ptr").count() >= 2);
}

#[test]
fn emits_async_main_entry_lifecycle_with_helper_task_handle_flows_in_program_mode() {
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

async fn other() -> Int {
return 2
}

fn schedule() -> Task[Int] {
return worker()
}

fn forward(task: Task[Int]) -> Task[Int] {
return task
}

async fn main() -> Int {
let direct = await schedule()

let bound = schedule()
let bound_value = await bound

let spawned = spawn schedule()
let spawned_value = await spawned

let task = other()
let forwarded = forward(task)
let forwarded_value = await forwarded

let next = worker()
let running = spawn forward(next)
let running_value = await running

return direct + bound_value + spawned_value + forwarded_value + running_value
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.contains("define i32 @main()"));
    assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
    assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
    assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
    assert!(rendered.matches("_schedule(").count() >= 3);
    assert!(rendered.matches("_forward(").count() >= 3);
    assert!(rendered.matches("@qlrt_executor_spawn").count() >= 4);
    assert!(rendered.matches("@qlrt_task_await").count() >= 6);
    assert!(!rendered.contains("does not support `await` lowering yet"));
    assert!(!rendered.contains("does not support `spawn` lowering yet"));
}

#[test]
fn emits_async_main_entry_lifecycle_with_zero_sized_helper_task_handle_flows_in_program_mode() {
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

async fn other() -> Wrap {
return Wrap { values: [] }
}

fn schedule() -> Task[Wrap] {
return worker()
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
return task
}

fn score(value: Wrap) -> Int {
return 1
}

async fn main() -> Int {
let direct = await schedule()

let bound = schedule()
let bound_value = await bound

let spawned = spawn schedule()
let spawned_value = await spawned

let task = other()
let forwarded = forward(task)
let forwarded_value = await forwarded

let next = worker()
let running = spawn forward(next)
let running_value = await running

return score(direct)
    + score(bound_value)
    + score(spawned_value)
    + score(forwarded_value)
    + score(running_value)
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.contains("define i32 @main()"));
    assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
    assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
    assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
    assert!(rendered.matches("_schedule(").count() >= 3);
    assert!(rendered.matches("_forward(").count() >= 3);
    assert!(rendered.matches("_score(").count() >= 6);
    assert!(rendered.matches("@qlrt_executor_spawn").count() >= 4);
    assert!(rendered.matches("@qlrt_task_await").count() >= 6);
    assert!(rendered.contains("load { [0 x i64] }, ptr %t"));
    assert!(!rendered.contains("does not support `await` lowering yet"));
    assert!(!rendered.contains("does not support `spawn` lowering yet"));
}

#[test]
fn emits_async_main_entry_lifecycle_with_local_returned_task_handle_helpers_in_program_mode() {
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
let task = worker()
return task
}

async fn main() -> Int {
let first = await schedule()
let second = await schedule()
return first + second
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.contains("define i32 @main()"));
    assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
    assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
    assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
    assert!(rendered.matches("_schedule(").count() >= 3);
    assert!(rendered.matches("@qlrt_task_await").count() >= 3);
    assert!(rendered.contains("store ptr %t"));
    assert!(rendered.contains("load i64, ptr %t"));
    assert!(!rendered.contains("does not support `await` lowering yet"));
}

#[test]
fn emits_async_main_entry_lifecycle_with_direct_task_handles_in_program_mode() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
async fn worker(value: Int) -> Int {
return value
}

async fn main() -> Int {
let first_task = worker(1)
let second_task = worker(2)
let first = await first_task
let second = await second_task
return first + second
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.contains("define i32 @main()"));
    assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
    assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
    assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
    assert!(rendered.matches("@qlrt_task_await").count() >= 3);
    assert!(rendered.matches("store ptr %t").count() >= 2);
    assert!(rendered.matches("load i64, ptr %t").count() >= 2);
    assert!(rendered.matches("call ptr @qlrt_task_await(ptr %t").count() >= 2);
    assert!(!rendered.contains("does not support `await` lowering yet"));
}

#[test]
fn emits_async_main_entry_lifecycle_with_spawned_bound_task_handles_in_program_mode() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
async fn worker(value: Int) -> Int {
return value
}

async fn main() -> Int {
let first_task = worker(1)
let second_task = worker(2)
let first_running = spawn first_task
let second_running = spawn second_task
let first = await first_running
let second = await second_running
return first + second
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.contains("define i32 @main()"));
    assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
    assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
    assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
    assert!(rendered.matches("@qlrt_executor_spawn").count() >= 3);
    assert!(rendered.matches("@qlrt_task_await").count() >= 3);
    assert!(rendered.matches("store ptr %t").count() >= 2);
    assert!(
        rendered
            .matches("call ptr @qlrt_executor_spawn(ptr null, ptr %t")
            .count()
            >= 2
    );
    assert!(rendered.matches("load i64, ptr %t").count() >= 2);
    assert!(!rendered.contains("does not support `spawn` lowering yet"));
}

#[test]
fn emits_async_main_entry_lifecycle_with_local_returned_zero_sized_task_handle_helpers_in_program_mode()
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

fn schedule() -> Task[Wrap] {
let task = worker()
return task
}

fn score(value: Wrap) -> Int {
return 1
}

async fn main() -> Int {
let first = await schedule()
let second = await schedule()
return score(first) + score(second)
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.contains("define i32 @main()"));
    assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
    assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
    assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
    assert!(rendered.matches("_schedule(").count() >= 3);
    assert!(rendered.matches("@qlrt_task_await").count() >= 3);
    assert!(rendered.contains("store ptr %t"));
    assert!(rendered.contains("load { [0 x i64] }, ptr %t"));
    assert!(rendered.matches("_score(").count() >= 3);
    assert!(!rendered.contains("does not support `await` lowering yet"));
}

#[test]
fn emits_async_main_entry_lifecycle_with_zero_sized_aggregate_results_in_program_mode() {
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

async fn empty_values() -> [Int; 0] {
return []
}

async fn wrapped() -> Wrap {
return Wrap { values: [] }
}

fn score(values: [Int; 0], value: Wrap) -> Int {
return 1
}

async fn main() -> Int {
let first = await empty_values()
let second = await wrapped()
return score(first, second)
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.contains("define i32 @main()"));
    assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
    assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
    assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
    assert!(rendered.matches("@qlrt_task_await").count() >= 3);
    assert!(rendered.contains("load { [0 x i64] }, ptr %t"));
    assert!(rendered.contains("load [0 x i64], ptr %t"));
    assert!(rendered.matches("_score(").count() >= 2);
    assert!(!rendered.contains("does not support `await` lowering yet"));
}

#[test]
fn emits_async_main_entry_lifecycle_with_spawned_zero_sized_aggregate_results_in_program_mode() {
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

fn score(value: Wrap) -> Int {
return 1
}

async fn main() -> Int {
let task = spawn worker()
let first = await task
return score(first)
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.contains("define i32 @main()"));
    assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
    assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
    assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
    assert!(rendered.matches("@qlrt_executor_spawn").count() >= 2);
    assert!(rendered.matches("@qlrt_task_await").count() >= 2);
    assert!(rendered.contains("load { [0 x i64] }, ptr %t"));
    assert!(rendered.matches("_score(").count() >= 2);
    assert!(!rendered.contains("does not support `spawn` lowering yet"));
}

#[test]
fn emits_async_main_entry_lifecycle_with_aggregate_results_in_program_mode() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
struct Pair {
left: Int,
right: Int,
}

async fn tuple_worker() -> (Bool, Int) {
return (true, 1)
}

async fn array_worker() -> [Int; 3] {
return [2, 3, 4]
}

async fn pair_worker() -> Pair {
return Pair { left: 5, right: 6 }
}

fn score_tuple(pair: (Bool, Int)) -> Int {
if pair[0] {
    return pair[1]
}
return 0
}

fn score_array(values: [Int; 3]) -> Int {
return values[0] + values[1] + values[2]
}

fn score_pair(pair: Pair) -> Int {
return pair.left + pair.right
}

async fn main() -> Int {
let tuple_value = await tuple_worker()
let array_value = await array_worker()
let pair_value = await pair_worker()
return score_tuple(tuple_value) + score_array(array_value) + score_pair(pair_value)
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.contains("define i32 @main()"));
    assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
    assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
    assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
    assert!(rendered.matches("@qlrt_task_await").count() >= 4);
    assert!(rendered.contains("load { i1, i64 }, ptr %t"));
    assert!(rendered.contains("load [3 x i64], ptr %t"));
    assert!(rendered.contains("load { i64, i64 }, ptr %t"));
    assert!(rendered.matches("_score_").count() >= 4);
    assert!(!rendered.contains("does not support `await` lowering yet"));
}

#[test]
fn emits_async_main_entry_lifecycle_with_spawned_aggregate_results_in_program_mode() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
struct Pair {
left: Int,
right: Int,
}

async fn tuple_worker() -> (Bool, Int) {
return (true, 1)
}

async fn array_worker() -> [Int; 3] {
return [2, 3, 4]
}

async fn pair_worker() -> Pair {
return Pair { left: 5, right: 6 }
}

fn score_tuple(pair: (Bool, Int)) -> Int {
if pair[0] {
    return pair[1]
}
return 0
}

fn score_array(values: [Int; 3]) -> Int {
return values[0] + values[1] + values[2]
}

fn score_pair(pair: Pair) -> Int {
return pair.left + pair.right
}

async fn main() -> Int {
let tuple_task = spawn tuple_worker()
let array_task = spawn array_worker()
let pair_task = spawn pair_worker()
let tuple_value = await tuple_task
let array_value = await array_task
let pair_value = await pair_task
return score_tuple(tuple_value) + score_array(array_value) + score_pair(pair_value)
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.contains("define i32 @main()"));
    assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
    assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
    assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
    assert!(rendered.matches("@qlrt_executor_spawn").count() >= 4);
    assert!(rendered.matches("@qlrt_task_await").count() >= 4);
    assert!(rendered.contains("load { i1, i64 }, ptr %t"));
    assert!(rendered.contains("load [3 x i64], ptr %t"));
    assert!(rendered.contains("load { i64, i64 }, ptr %t"));
    assert!(rendered.matches("_score_").count() >= 4);
    assert!(!rendered.contains("does not support `spawn` lowering yet"));
}

#[test]
fn emits_async_main_entry_lifecycle_with_recursive_aggregate_results_in_program_mode() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
struct Pair {
left: Int,
right: Int,
}

async fn worker() -> (Pair, [Int; 2]) {
return (Pair { left: 1, right: 2 }, [3, 4])
}

fn score(result: (Pair, [Int; 2])) -> Int {
return result[0].left + result[0].right + result[1][0] + result[1][1]
}

async fn main() -> Int {
let value = await worker()
return score(value)
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.contains("define i32 @main()"));
    assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
    assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
    assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
    assert!(rendered.matches("@qlrt_task_await").count() >= 2);
    assert!(rendered.contains("load { { i64, i64 }, [2 x i64] }, ptr %t"));
    assert!(rendered.matches("_score(").count() >= 2);
    assert!(!rendered.contains("does not support `await` lowering yet"));
}

#[test]
fn emits_async_main_entry_lifecycle_with_spawned_recursive_aggregate_results_in_program_mode() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
struct Pair {
left: Int,
right: Int,
}

async fn worker() -> (Pair, [Int; 2]) {
return (Pair { left: 1, right: 2 }, [3, 4])
}

fn score(result: (Pair, [Int; 2])) -> Int {
return result[0].left + result[0].right + result[1][0] + result[1][1]
}

async fn main() -> Int {
let task = spawn worker()
let value = await task
return score(value)
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.contains("define i32 @main()"));
    assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
    assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
    assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
    assert!(rendered.matches("@qlrt_executor_spawn").count() >= 2);
    assert!(rendered.matches("@qlrt_task_await").count() >= 2);
    assert!(rendered.contains("load { { i64, i64 }, [2 x i64] }, ptr %t"));
    assert!(rendered.matches("_score(").count() >= 2);
    assert!(!rendered.contains("does not support `spawn` lowering yet"));
}

#[test]
fn emits_async_recursive_aggregate_param_lowering_in_program_mode() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
struct Pair {
left: Int,
right: Int,
}

async fn worker(pair: Pair, values: [Int; 2]) -> Int {
return pair.right + values[1]
}

async fn main() -> Int {
return await worker(Pair { left: 1, right: 2 }, [3, 4])
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.contains("define ptr @ql_1_worker({ i64, i64 } %arg0, [2 x i64] %arg1)"));
    assert!(rendered.contains("call ptr @qlrt_async_frame_alloc(i64 32, i64 8)"));
    assert!(rendered.contains("store { i64, i64 } %arg0, ptr %async_frame_field0"));
    assert!(rendered.contains("store [2 x i64] %arg1, ptr %async_frame_field1"));
    assert!(rendered.contains("call ptr @ql_1_worker("));
    assert!(rendered.contains("call ptr @qlrt_task_await(ptr"));
    assert!(rendered.contains("load i64, ptr"));
    assert!(!rendered.contains("does not support `await` lowering yet"));
}

#[test]
fn emits_async_spawned_recursive_aggregate_param_lowering_in_program_mode() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
struct Pair {
left: Int,
right: Int,
}

async fn worker(pair: Pair, values: [Int; 2]) -> Int {
return pair.right + values[1]
}

async fn main() -> Int {
let task = spawn worker(Pair { left: 1, right: 2 }, [3, 4])
return await task
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.contains("define ptr @ql_1_worker({ i64, i64 } %arg0, [2 x i64] %arg1)"));
    assert!(rendered.contains("call ptr @qlrt_async_frame_alloc(i64 32, i64 8)"));
    assert!(rendered.contains("store { i64, i64 } %arg0, ptr %async_frame_field0"));
    assert!(rendered.contains("store [2 x i64] %arg1, ptr %async_frame_field1"));
    assert!(rendered.contains("call ptr @ql_1_worker("));
    assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
    assert!(rendered.matches("@qlrt_executor_spawn").count() >= 2);
    assert!(rendered.matches("@qlrt_task_await").count() >= 2);
    assert!(rendered.contains("load i64, ptr"));
    assert!(!rendered.contains("does not support `spawn` lowering yet"));
}

#[test]
fn emits_async_zero_sized_recursive_aggregate_param_lowering_in_program_mode() {
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

async fn worker(values: [Int; 0], wrap: Wrap, nested: [[Int; 0]; 1]) -> Int {
return 7
}

async fn main() -> Int {
return await worker([], Wrap { values: [] }, [[]])
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.contains(
        "define ptr @ql_1_worker([0 x i64] %arg0, { [0 x i64] } %arg1, [1 x [0 x i64]] %arg2)"
    ));
    assert!(rendered.contains("call ptr @qlrt_async_frame_alloc(i64 0, i64 8)"));
    assert!(rendered.contains("call ptr @ql_1_worker("));
    assert!(rendered.contains("[0 x i64] zeroinitializer"));
    assert!(rendered.contains("[1 x [0 x i64]]"));
    assert!(rendered.contains("call ptr @qlrt_task_await(ptr"));
    assert!(rendered.contains("load i64, ptr"));
    assert!(!rendered.contains("does not support `await` lowering yet"));
}

#[test]
fn emits_async_spawned_zero_sized_recursive_aggregate_param_lowering_in_program_mode() {
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

async fn worker(values: [Int; 0], wrap: Wrap, nested: [[Int; 0]; 1]) -> Int {
return 7
}

async fn main() -> Int {
let task = spawn worker([], Wrap { values: [] }, [[]])
return await task
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.contains(
        "define ptr @ql_1_worker([0 x i64] %arg0, { [0 x i64] } %arg1, [1 x [0 x i64]] %arg2)"
    ));
    assert!(rendered.contains("call ptr @qlrt_async_frame_alloc(i64 0, i64 8)"));
    assert!(rendered.contains("call ptr @ql_1_worker("));
    assert!(rendered.contains("[0 x i64] zeroinitializer"));
    assert!(rendered.contains("[1 x [0 x i64]]"));
    assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
    assert!(rendered.matches("@qlrt_executor_spawn").count() >= 2);
    assert!(rendered.matches("@qlrt_task_await").count() >= 2);
    assert!(rendered.contains("load i64, ptr"));
    assert!(!rendered.contains("does not support `spawn` lowering yet"));
}

#[test]
fn emits_async_main_entry_lifecycle_with_projected_task_handle_awaits_in_program_mode() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
struct TaskPair {
left: Task[Int],
right: Task[Int],
}

async fn worker(value: Int) -> Int {
return value
}

fn score(value: Int) -> Int {
return value
}

async fn main() -> Int {
let tuple = (worker(1), worker(2))
let tuple_first = await tuple[0]
let tuple_second = await tuple[1]

let array = [worker(3), worker(4)]
let array_first = await array[0]
let array_second = await array[1]

let pair = TaskPair { left: worker(5), right: worker(6) }
let struct_first = await pair.left
let struct_second = await pair.right

return score(tuple_first)
    + score(tuple_second)
    + score(array_first)
    + score(array_second)
    + score(struct_first)
    + score(struct_second)
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.contains("define i32 @main()"));
    assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
    assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
    assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
    assert!(rendered.matches("@qlrt_task_await").count() >= 7);
    assert!(
        rendered
            .matches("getelementptr inbounds { ptr, ptr }, ptr")
            .count()
            >= 4
    );
    assert!(rendered.matches("_score(").count() >= 6);
    assert!(!rendered.contains("does not support `await` lowering yet"));
}

#[test]
fn emits_async_main_entry_lifecycle_with_projected_task_handle_spawns_in_program_mode() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
struct TaskPair {
left: Task[Int],
right: Task[Int],
}

async fn worker(value: Int) -> Int {
return value
}

fn score(value: Int) -> Int {
return value
}

async fn main() -> Int {
let tuple = (worker(1), worker(2))
let tuple_running = spawn tuple[0]
let tuple_value = await tuple_running

let array = [worker(3), worker(4)]
let array_running = spawn array[0]
let array_value = await array_running

let pair = TaskPair { left: worker(5), right: worker(6) }
let struct_running = spawn pair.left
let struct_value = await struct_running

return score(tuple_value) + score(array_value) + score(struct_value)
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.contains("define i32 @main()"));
    assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
    assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
    assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
    assert!(rendered.matches("@qlrt_executor_spawn").count() >= 4);
    assert!(rendered.matches("@qlrt_task_await").count() >= 4);
    assert!(
        rendered
            .matches("getelementptr inbounds { ptr, ptr }, ptr")
            .count()
            >= 2
    );
    assert!(rendered.contains("getelementptr inbounds [2 x ptr], ptr"));
    assert!(rendered.matches("load i64, ptr %t").count() >= 3);
    assert!(rendered.matches("_score(").count() >= 4);
    assert!(!rendered.contains("does not support field or index projections yet"));
    assert!(!rendered.contains("does not support `spawn` lowering yet"));
}

#[test]
fn emits_async_main_entry_lifecycle_with_projected_task_handle_reinit_in_program_mode() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
struct TaskPair {
left: Task[Int],
right: Task[Int],
}

async fn worker(value: Int) -> Int {
return value
}

fn score(value: Int) -> Int {
return value
}

async fn main() -> Int {
var tuple = (worker(1), worker(2))
let tuple_first = await tuple[0]
tuple[0] = worker(7)
let tuple_second = await tuple[0]

var array = [worker(3), worker(4)]
let array_first = await array[0]
array[0] = worker(8)
let array_second = await array[0]

var pair = TaskPair { left: worker(5), right: worker(6) }
let struct_first = await pair.left
pair.left = worker(9)
let struct_second = await pair.left

return score(tuple_first)
    + score(tuple_second)
    + score(array_first)
    + score(array_second)
    + score(struct_first)
    + score(struct_second)
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.contains("define i32 @main()"));
    assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
    assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
    assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
    assert!(rendered.matches("@qlrt_task_await").count() >= 7);
    assert!(
        rendered
            .matches("getelementptr inbounds { ptr, ptr }, ptr")
            .count()
            >= 2
    );
    assert!(rendered.contains("getelementptr inbounds [2 x ptr], ptr"));
    assert!(rendered.matches("store ptr").count() >= 9);
    assert!(rendered.matches("load i64, ptr %t").count() >= 6);
    assert!(rendered.matches("_score(").count() >= 7);
    assert!(!rendered.contains("does not support field or index projections yet"));
}

#[test]
fn emits_async_main_entry_lifecycle_with_projected_task_handle_conditional_reinit_in_program_mode()
{
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
async fn worker(value: Int) -> Int {
return value
}

fn score(value: Int) -> Int {
return value
}

async fn main() -> Int {
let flag = true
var tasks = [worker(1), worker(2)]
if flag {
    let first = await tasks[0]
    tasks[0] = worker(7)
}
let final_value = await tasks[0]
return score(final_value)
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.contains("define i32 @main()"));
    assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
    assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
    assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
    assert!(rendered.matches("@qlrt_task_await").count() >= 3);
    assert!(
        rendered
            .matches("getelementptr inbounds [2 x ptr], ptr")
            .count()
            >= 2
    );
    assert!(rendered.contains("store ptr %t"));
    assert!(rendered.matches("load i64, ptr %t").count() >= 2);
    assert!(rendered.matches("_score(").count() >= 2);
    assert!(!rendered.contains("does not support field or index projections yet"));
    assert!(!rendered.contains("does not support assignment to field or index projections yet"));
}

#[test]
fn emits_async_main_entry_lifecycle_with_projected_dynamic_task_handle_reinit_in_program_mode() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
struct Slot {
value: Int,
}

async fn worker(value: Int) -> Int {
return value
}

fn score(value: Int) -> Int {
return value
}

async fn main() -> Int {
var tasks = [worker(1), worker(2)]
let slot = Slot { value: 0 }
let first = await tasks[slot.value]
tasks[slot.value] = worker(first + 1)
let second = await tasks[slot.value]
return score(first) + score(second)
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.contains("define i32 @main()"));
    assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
    assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
    assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
    assert!(rendered.matches("@qlrt_task_await").count() >= 3);
    assert!(
        rendered
            .matches("getelementptr inbounds [2 x ptr], ptr")
            .count()
            >= 3
    );
    assert!(rendered.matches("store ptr").count() >= 4);
    assert!(rendered.matches("_score(").count() >= 3);
    assert!(!rendered.contains("does not support field or index projections yet"));
    assert!(!rendered.contains("does not support assignment to field or index projections yet"));
}
