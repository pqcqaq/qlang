use super::*;

#[test]
fn emits_async_main_entry_lifecycle_with_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_task_handle_queued_root_chain_inline_forward_await_in_program_mode()
 {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
struct Pending {
tasks: [Task[Int]; 2],
}

struct Slot {
value: Int,
}

struct Bundle {
tasks: [Task[Int]; 2],
}

struct Envelope {
bundle: Bundle,
tail: Task[Int],
}

const INDEX: Int = 0

async fn worker(value: Int) -> Int {
return value
}

fn forward(task: Task[Int]) -> Task[Int] {
return task
}

async fn main() -> Int {
let row_root = INDEX
let row = row_root
let slots = [row, row]
let slot_root = slots
let slot_alias_root = slot_root
let alias_slots = slot_alias_root
var pending = Pending {
    tasks: [worker(8), worker(14)],
}
let root = pending.tasks
let root_alias = root
let alias = root_alias
let slot = Slot { value: INDEX }
let slot_alias = slot
if slot_alias.value == 0 {
    let first = await alias[alias_slots[row]]
    pending.tasks[slots[row]] = worker(first + 35)
}
let tail_tasks = pending.tasks
let forwarded = forward(alias[alias_slots[row]])
let running_task = forwarded
let env = Envelope {
    bundle: Bundle {
        tasks: [running_task, worker(67)],
    },
    tail: tail_tasks[1],
}
let queue_root = env.bundle.tasks
let queue_alias_root = queue_root
let queued_tasks = queue_alias_root
let second = await forward(queued_tasks[0])
let extra = await env.bundle.tasks[1]
let tail = await env.tail
return second + extra + tail
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.contains("define i32 @main()"));
    assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
    assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
    assert!(rendered.matches("@qlrt_executor_spawn").count() >= 1);
    assert!(rendered.matches("@qlrt_task_await").count() >= 5);
    assert!(rendered.matches("_forward(").count() >= 3);
    assert!(
        rendered
            .matches("getelementptr inbounds { i64 }, ptr")
            .count()
            >= 2
    );
    assert!(rendered.matches("store ptr").count() >= 10);
    assert!(!rendered.contains("does not support field or index projections yet"));
    assert!(!rendered.contains("does not support assignment to field or index projections yet"));
}

#[test]
fn emits_async_main_entry_lifecycle_with_dynamic_task_handle_array_index_assignment_in_program_mode()
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
var index = 0
var tasks = [worker(1), worker(2)]
tasks[index] = worker(3)
let value = await tasks[0]
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
    assert!(
        rendered
            .matches("getelementptr inbounds [2 x ptr], ptr")
            .count()
            >= 2
    );
    assert!(rendered.matches("store ptr").count() >= 4);
    assert!(rendered.matches("load i64, ptr %t").count() >= 1);
    assert!(rendered.matches("_score(").count() >= 2);
    assert!(!rendered.contains("does not support field or index projections yet"));
    assert!(!rendered.contains("does not support assignment to field or index projections yet"));
}

#[test]
fn emits_async_main_entry_lifecycle_with_dynamic_task_handle_spawn_and_sibling_task_use_in_program_mode()
 {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
struct Pending {
tasks: [Task[Int]; 2],
fallback: Task[Int],
}

async fn worker(value: Int) -> Int {
return value
}

fn score(value: Int) -> Int {
return value
}

async fn main() -> Int {
var index = 0
let pending = Pending {
    tasks: [worker(1), worker(2)],
    fallback: worker(7),
}
let running = spawn pending.tasks[index]
let first = await running
let second = await pending.fallback
return score(first) + score(second)
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.contains("define i32 @main()"));
    assert!(rendered.matches("@qlrt_executor_spawn").count() >= 2);
    assert!(rendered.matches("@qlrt_task_await").count() >= 3);
    assert!(
        rendered
            .matches("getelementptr inbounds [2 x ptr], ptr")
            .count()
            >= 1
    );
    assert!(rendered.matches("load i64, ptr %t").count() >= 2);
    assert!(rendered.matches("_score(").count() >= 3);
    assert!(!rendered.contains("does not support field or index projections yet"));
    assert!(!rendered.contains("does not support assignment to field or index projections yet"));
}

#[test]
fn emits_async_main_entry_lifecycle_with_zero_sized_nested_task_handle_payload_in_program_mode() {
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

async fn outer() -> Task[Wrap] {
return worker()
}

fn score(value: Wrap) -> Int {
return 1
}

async fn main() -> Int {
let next = await outer()
let value = await next
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
    assert!(rendered.matches("@qlrt_task_await").count() >= 3);
    assert!(rendered.contains("load ptr, ptr %t"));
    assert!(rendered.contains("load { [0 x i64] }, ptr %t"));
    assert!(rendered.matches("_score(").count() >= 2);
    assert!(!rendered.contains("does not support `await` lowering yet"));
}

#[test]
fn emits_async_main_entry_lifecycle_with_zero_sized_struct_task_handle_payload_in_program_mode() {
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

fn score(value: Wrap) -> Int {
return 1
}

async fn main() -> Int {
let pending = await outer()
let first = await pending.first
let second = await pending.second
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
    assert!(rendered.matches("@qlrt_task_await").count() >= 4);
    assert!(rendered.contains("load { ptr, ptr }, ptr %t"));
    assert!(rendered.contains("getelementptr inbounds { ptr, ptr }, ptr"));
    assert!(rendered.contains("load { [0 x i64] }, ptr %t"));
    assert!(rendered.matches("_score(").count() >= 3);
    assert!(!rendered.contains("does not support `await` lowering yet"));
}

#[test]
fn emits_async_main_entry_lifecycle_with_zero_sized_projected_task_handle_awaits_in_program_mode() {
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
left: Task[Wrap],
right: Task[Wrap],
}

async fn worker() -> Wrap {
return Wrap { values: [] }
}

fn score(value: Wrap) -> Int {
return 1
}

async fn main() -> Int {
let tuple = (worker(), worker())
let tuple_first = await tuple[0]
let tuple_second = await tuple[1]

let array = [worker(), worker()]
let array_first = await array[0]
let array_second = await array[1]

let pair = TaskPair { left: worker(), right: worker() }
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
    assert!(
        rendered
            .matches("getelementptr inbounds [2 x ptr], ptr")
            .count()
            >= 2
    );
    assert!(rendered.matches("load { [0 x i64] }, ptr %t").count() >= 6);
    assert!(rendered.matches("_score(").count() >= 7);
    assert!(!rendered.contains("does not support field or index projections yet"));
    assert!(!rendered.contains("does not support `await` lowering yet"));
}

#[test]
fn emits_async_main_entry_lifecycle_with_zero_sized_projected_task_handle_spawns_in_program_mode() {
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
left: Task[Wrap],
right: Task[Wrap],
}

async fn worker() -> Wrap {
return Wrap { values: [] }
}

fn score(value: Wrap) -> Int {
return 1
}

async fn main() -> Int {
let tuple = (worker(), worker())
let tuple_running = spawn tuple[0]
let tuple_value = await tuple_running

let array = [worker(), worker()]
let array_running = spawn array[0]
let array_value = await array_running

let pair = TaskPair { left: worker(), right: worker() }
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
    assert!(rendered.matches("load { [0 x i64] }, ptr %t").count() >= 3);
    assert!(rendered.matches("_score(").count() >= 4);
    assert!(!rendered.contains("does not support field or index projections yet"));
    assert!(!rendered.contains("does not support `spawn` lowering yet"));
}

#[test]
fn emits_async_main_entry_lifecycle_with_zero_sized_projected_task_handle_reinit_in_program_mode() {
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
left: Task[Wrap],
right: Task[Wrap],
}

async fn worker() -> Wrap {
return Wrap { values: [] }
}

fn score(value: Wrap) -> Int {
return 1
}

async fn main() -> Int {
var tuple = (worker(), worker())
let tuple_first = await tuple[0]
tuple[0] = worker()
let tuple_second = await tuple[0]

var array = [worker(), worker()]
let array_first = await array[0]
array[0] = worker()
let array_second = await array[0]

var pair = TaskPair { left: worker(), right: worker() }
let struct_first = await pair.left
pair.left = worker()
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
            >= 4
    );
    assert!(
        rendered
            .matches("getelementptr inbounds [2 x ptr], ptr")
            .count()
            >= 2
    );
    assert!(rendered.contains("store ptr %t"));
    assert!(rendered.matches("load { [0 x i64] }, ptr %t").count() >= 6);
    assert!(rendered.matches("_score(").count() >= 7);
    assert!(!rendered.contains("does not support field or index projections yet"));
    assert!(!rendered.contains("does not support assignment to field or index projections yet"));
}

#[test]
fn emits_async_main_entry_lifecycle_with_zero_sized_projected_task_handle_conditional_reinit_in_program_mode()
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

fn score(value: Wrap) -> Int {
return 1
}

async fn main() -> Int {
let flag = true
var tasks = [worker(), worker()]
if flag {
    let first = await tasks[0]
    tasks[0] = worker()
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
    assert!(rendered.matches("load { [0 x i64] }, ptr %t").count() >= 2);
    assert!(rendered.matches("_score(").count() >= 2);
    assert!(!rendered.contains("does not support field or index projections yet"));
    assert!(!rendered.contains("does not support assignment to field or index projections yet"));
}

#[test]
fn emits_async_main_entry_lifecycle_with_branch_spawned_reinit_in_program_mode() {
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

async fn fresh_worker() -> Int {
return 2
}

fn score(value: Int) -> Int {
return value
}

async fn main() -> Int {
let flag = true
var task = worker()
if flag {
    let running = spawn task
    task = fresh_worker()
    let first = await running
    return score(first)
} else {
    task = fresh_worker()
}
let final_value = await task
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
    assert!(rendered.matches("@qlrt_executor_spawn").count() >= 2);
    assert!(rendered.matches("@qlrt_task_await").count() >= 2);
    assert!(rendered.contains("store ptr %t"));
    assert!(rendered.contains("load i64, ptr %t"));
    assert!(rendered.matches("_score(").count() >= 2);
    assert!(!rendered.contains("does not support `spawn` lowering yet"));
}

#[test]
fn emits_async_main_entry_lifecycle_with_zero_sized_branch_spawned_reinit_in_program_mode() {
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

fn score(value: Wrap) -> Int {
return 1
}

async fn main() -> Int {
let flag = true
var task = worker()
if flag {
    let running = spawn task
    task = fresh_worker()
    let first = await running
    return score(first)
} else {
    task = fresh_worker()
}
let final_value = await task
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
    assert!(rendered.matches("@qlrt_executor_spawn").count() >= 2);
    assert!(rendered.matches("@qlrt_task_await").count() >= 2);
    assert!(rendered.contains("store ptr %t"));
    assert!(rendered.contains("load { [0 x i64] }, ptr %t"));
    assert!(rendered.matches("_score(").count() >= 2);
    assert!(!rendered.contains("does not support `spawn` lowering yet"));
}

#[test]
fn emits_async_main_entry_lifecycle_with_zero_sized_reverse_branch_spawned_reinit_in_program_mode()
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

fn score(value: Wrap) -> Int {
return 1
}

async fn main() -> Int {
let flag = true
var task = worker()
if flag {
    task = fresh_worker()
} else {
    let running = spawn task
    task = fresh_worker()
    let first = await running
    return score(first)
}
let final_value = await task
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
    assert!(rendered.matches("@qlrt_executor_spawn").count() >= 2);
    assert!(rendered.matches("@qlrt_task_await").count() >= 2);
    assert!(rendered.contains("store ptr %t"));
    assert!(rendered.contains("load { [0 x i64] }, ptr %t"));
    assert!(rendered.matches("_score(").count() >= 2);
    assert!(!rendered.contains("does not support `spawn` lowering yet"));
}

#[test]
fn emits_async_main_entry_lifecycle_with_conditional_async_call_spawns_in_program_mode() {
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

async fn choose(flag: Bool) -> Int {
if flag {
    let running = spawn worker();
    return await running
}
return await worker()
}

async fn choose_reverse(flag: Bool) -> Int {
if flag {
    return await worker()
}
let running = spawn worker();
return await running
}

fn score(value: Int) -> Int {
return value
}

async fn main() -> Int {
let first = await choose(true)
let second = await choose_reverse(false)
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
    assert!(rendered.matches("@qlrt_executor_spawn").count() >= 3);
    assert!(rendered.matches("@qlrt_task_await").count() >= 5);
    assert!(rendered.contains("load i64, ptr %t"));
    assert!(rendered.matches("_score(").count() >= 3);
    assert!(!rendered.contains("does not support `spawn` lowering yet"));
}

#[test]
fn emits_async_main_entry_lifecycle_with_zero_sized_conditional_async_call_spawns_in_program_mode()
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
    let running = spawn worker();
    return await running
}
return await worker()
}

async fn choose_reverse(flag: Bool) -> Wrap {
if flag {
    return await worker()
}
let running = spawn worker();
return await running
}

fn score(value: Wrap) -> Int {
return 1
}

async fn main() -> Int {
let first = await choose(true)
let second = await choose_reverse(false)
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
    assert!(rendered.matches("@qlrt_executor_spawn").count() >= 3);
    assert!(rendered.matches("@qlrt_task_await").count() >= 5);
    assert!(rendered.contains("load { [0 x i64] }, ptr %t"));
    assert!(rendered.matches("_score(").count() >= 3);
    assert!(!rendered.contains("does not support `spawn` lowering yet"));
}

#[test]
fn emits_async_main_entry_lifecycle_with_conditional_helper_task_handle_spawns_in_program_mode() {
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

async fn choose(flag: Bool, task: Task[Int]) -> Int {
if flag {
    let running = spawn task
    return await running
}
return await task
}

async fn choose_reverse(flag: Bool, task: Task[Int]) -> Int {
if flag {
    return await task
}
let running = spawn task
return await running
}

async fn helper(flag: Bool) -> Int {
return await choose(flag, worker())
}

async fn helper_reverse(flag: Bool) -> Int {
return await choose_reverse(flag, worker())
}

fn score(value: Int) -> Int {
return value
}

async fn main() -> Int {
let first = await helper(true)
let second = await helper_reverse(false)
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
    assert!(rendered.matches("@qlrt_executor_spawn").count() >= 3);
    assert!(rendered.matches("@qlrt_task_await").count() >= 7);
    assert!(rendered.contains("load i64, ptr %t"));
    assert!(rendered.matches("_score(").count() >= 3);
    assert!(!rendered.contains("does not support `spawn` lowering yet"));
}

#[test]
fn emits_async_main_entry_lifecycle_with_zero_sized_conditional_helper_task_handle_spawns_in_program_mode()
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
    let running = spawn task
    return await running
}
return await task
}

async fn choose_reverse(flag: Bool, task: Task[Wrap]) -> Wrap {
if flag {
    return await task
}
let running = spawn task
return await running
}

async fn helper(flag: Bool) -> Wrap {
return await choose(flag, worker())
}

async fn helper_reverse(flag: Bool) -> Wrap {
return await choose_reverse(flag, worker())
}

fn score(value: Wrap) -> Int {
return 1
}

async fn main() -> Int {
let first = await helper(true)
let second = await helper_reverse(false)
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
    assert!(rendered.matches("@qlrt_executor_spawn").count() >= 3);
    assert!(rendered.matches("@qlrt_task_await").count() >= 7);
    assert!(rendered.contains("load { [0 x i64] }, ptr %t"));
    assert!(rendered.matches("_score(").count() >= 3);
    assert!(!rendered.contains("does not support `spawn` lowering yet"));
}

#[test]
fn rejects_async_main_without_required_executor_spawn_hook() {
    // async fn main requires the executor-spawn hook; omitting it must error.
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        // TaskSpawn (executor-spawn) intentionally absent.
        RuntimeCapability::TaskAwait,
    ]);
    let messages = emit_error_with_runtime_hooks(
        r#"
async fn main() -> Int {
return 1
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(
        messages.iter().any(|m| m.contains("executor-spawn")),
        "expected executor-spawn hook error, got: {messages:?}"
    );
}
