use super::*;

#[test]
fn emits_async_main_entry_lifecycle_with_projected_dynamic_task_handle_conditional_reinit_in_program_mode()
 {
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
let flag = true
var tasks = [worker(1), worker(2)]
let slot = Slot { value: 0 }
if flag {
    let first = await tasks[slot.value]
    tasks[slot.value] = worker(first + 1)
}
let final_value = await tasks[slot.value]
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
            >= 3
    );
    assert!(rendered.matches("store ptr").count() >= 4);
    assert!(rendered.matches("load i64, ptr %t").count() >= 2);
    assert!(rendered.matches("_score(").count() >= 2);
    assert!(!rendered.contains("does not support field or index projections yet"));
    assert!(!rendered.contains("does not support assignment to field or index projections yet"));
}

#[test]
fn emits_async_main_entry_lifecycle_with_guard_refined_dynamic_task_handle_literal_reinit_in_program_mode()
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

async fn helper(index: Int) -> Int {
var tasks = [worker(1), worker(2)]
if index == 0 {
    let first = await tasks[index]
    tasks[0] = worker(first + 1)
}
let final_value = await tasks[0]
return score(final_value)
}

async fn main() -> Int {
return await helper(0)
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
    assert!(rendered.matches("store ptr").count() >= 4);
    assert!(rendered.matches("load i64, ptr %t").count() >= 2);
    assert!(rendered.matches("_score(").count() >= 2);
    assert!(!rendered.contains("does not support field or index projections yet"));
    assert!(!rendered.contains("does not support assignment to field or index projections yet"));
}

#[test]
fn emits_async_main_entry_lifecycle_with_guard_refined_projected_dynamic_task_handle_literal_reinit_in_program_mode()
 {
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
if slot.value == 0 {
    let first = await tasks[slot.value]
    tasks[0] = worker(first + 1)
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
    assert!(
        rendered
            .matches("getelementptr inbounds { i64 }, ptr")
            .count()
            >= 1
    );
    assert!(rendered.matches("store ptr").count() >= 4);
    assert!(rendered.matches("load i64, ptr %t").count() >= 2);
    assert!(rendered.matches("_score(").count() >= 2);
    assert!(!rendered.contains("does not support field or index projections yet"));
    assert!(!rendered.contains("does not support assignment to field or index projections yet"));
}

#[test]
fn emits_async_main_entry_lifecycle_with_projected_root_dynamic_task_handle_reinit_in_program_mode()
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

async fn worker(value: Int) -> Int {
return value
}

async fn main() -> Int {
var pending = Pending {
    tasks: [worker(1), worker(2)],
}
let slot = Slot { value: 0 }
let first = await pending.tasks[slot.value]
pending.tasks[slot.value] = worker(first + 1)
return await pending.tasks[slot.value]
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
            .matches("getelementptr inbounds { [2 x ptr] }, ptr")
            .count()
            >= 3
    );
    assert!(
        rendered
            .matches("getelementptr inbounds [2 x ptr], ptr")
            .count()
            >= 3
    );
    assert!(rendered.matches("store ptr").count() >= 6);
    assert!(!rendered.contains("does not support field or index projections yet"));
    assert!(!rendered.contains("does not support assignment to field or index projections yet"));
}

#[test]
fn emits_async_main_entry_lifecycle_with_composed_dynamic_task_handle_reinit_in_program_mode() {
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

fn choose() -> Int {
return 0
}

fn score(value: Int) -> Int {
return value
}

async fn main() -> Int {
let row = choose()
var tasks = [worker(1), worker(2)]
let slots = [row, row]
let first = await tasks[slots[row]]
tasks[slots[row]] = worker(first + 1)
let final_value = await tasks[slots[row]]
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
            .matches("getelementptr inbounds [2 x i64], ptr")
            .count()
            >= 3
    );
    assert!(
        rendered
            .matches("getelementptr inbounds [2 x ptr], ptr")
            .count()
            >= 3
    );
    assert!(rendered.matches("store ptr").count() >= 5);
    assert!(rendered.matches("_score(").count() >= 2);
    assert!(!rendered.contains("does not support field or index projections yet"));
    assert!(!rendered.contains("does not support assignment to field or index projections yet"));
}

#[test]
fn emits_async_main_entry_lifecycle_with_alias_sourced_composed_dynamic_task_handle_reinit_in_program_mode()
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

fn choose() -> Int {
return 0
}

fn score(value: Int) -> Int {
return value
}

async fn main() -> Int {
let row = choose()
var tasks = [worker(1), worker(2)]
let slots = [row, row]
let alias = slots
let first = await tasks[alias[row]]
tasks[slots[row]] = worker(first + 1)
let final_value = await tasks[alias[row]]
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
            .matches("getelementptr inbounds [2 x i64], ptr")
            .count()
            >= 3
    );
    assert!(rendered.matches("store [2 x i64]").count() >= 2);
    assert!(rendered.matches("store ptr").count() >= 5);
    assert!(rendered.matches("_score(").count() >= 2);
    assert!(!rendered.contains("does not support field or index projections yet"));
    assert!(!rendered.contains("does not support assignment to field or index projections yet"));
}

#[test]
fn emits_async_main_entry_lifecycle_with_projected_root_const_backed_dynamic_task_handle_reinit_in_program_mode()
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

const INDEX: Int = 0

async fn worker(value: Int) -> Int {
return value
}

async fn main() -> Int {
var pending = Pending {
    tasks: [worker(8), worker(13)],
}
let first = await pending.tasks[INDEX]
pending.tasks[0] = worker(first + 3)
let second = await pending.tasks[INDEX]
let tail = await pending.tasks[1]
return second + tail
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
    assert!(
        rendered
            .matches("getelementptr inbounds { [2 x ptr] }, ptr")
            .count()
            >= 3
    );
    assert!(
        rendered
            .matches("getelementptr inbounds [2 x ptr], ptr")
            .count()
            >= 4
    );
    assert!(rendered.matches("store ptr").count() >= 5);
    assert!(!rendered.contains("does not support field or index projections yet"));
    assert!(!rendered.contains("does not support assignment to field or index projections yet"));
}

#[test]
fn emits_async_main_entry_lifecycle_with_aliased_projected_root_dynamic_task_handle_reinit_in_program_mode()
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

async fn worker(value: Int) -> Int {
return value
}

async fn main() -> Int {
var pending = Pending {
    tasks: [worker(5), worker(8)],
}
let slot = Slot { value: 0 }
let alias = pending.tasks
let first = await alias[slot.value]
pending.tasks[slot.value] = worker(first + 4)
let second = await alias[slot.value]
let tail = await pending.tasks[1]
return second + tail
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
    assert!(
        rendered
            .matches("getelementptr inbounds { [2 x ptr] }, ptr")
            .count()
            >= 4
    );
    assert!(
        rendered
            .matches("getelementptr inbounds [2 x ptr], ptr")
            .count()
            >= 4
    );
    assert!(
        rendered
            .matches("getelementptr inbounds { i64 }, ptr")
            .count()
            >= 2
    );
    assert!(rendered.matches("store ptr").count() >= 5);
    assert!(!rendered.contains("does not support field or index projections yet"));
    assert!(!rendered.contains("does not support assignment to field or index projections yet"));
}

#[test]
fn emits_async_main_entry_lifecycle_with_aliased_projected_root_const_backed_dynamic_task_handle_reinit_in_program_mode()
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

const INDEX: Int = 0

async fn worker(value: Int) -> Int {
return value
}

async fn main() -> Int {
var pending = Pending {
    tasks: [worker(6), worker(9)],
}
let alias = pending.tasks
let first = await alias[INDEX]
pending.tasks[0] = worker(first + 2)
let second = await alias[INDEX]
let tail = await pending.tasks[1]
return second + tail
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
    assert!(
        rendered
            .matches("getelementptr inbounds { [2 x ptr] }, ptr")
            .count()
            >= 4
    );
    assert!(
        rendered
            .matches("getelementptr inbounds [2 x ptr], ptr")
            .count()
            >= 4
    );
    assert!(rendered.matches("store ptr").count() >= 5);
    assert!(!rendered.contains("does not support field or index projections yet"));
    assert!(!rendered.contains("does not support assignment to field or index projections yet"));
}

#[test]
fn emits_async_main_entry_lifecycle_with_aliased_guard_refined_projected_root_dynamic_task_handle_reinit_in_program_mode()
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

async fn worker(value: Int) -> Int {
return value
}

async fn main() -> Int {
var pending = Pending {
    tasks: [worker(7), worker(11)],
}
let slot = Slot { value: 0 }
let alias = pending.tasks
if slot.value == 0 {
    let first = await alias[slot.value]
    pending.tasks[0] = worker(first + 3)
}
let second = await alias[0]
let tail = await pending.tasks[1]
return second + tail
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
    assert!(
        rendered
            .matches("getelementptr inbounds { [2 x ptr] }, ptr")
            .count()
            >= 4
    );
    assert!(
        rendered
            .matches("getelementptr inbounds [2 x ptr], ptr")
            .count()
            >= 4
    );
    assert!(
        rendered
            .matches("getelementptr inbounds { i64 }, ptr")
            .count()
            >= 2
    );
    assert!(rendered.matches("store ptr").count() >= 5);
    assert!(!rendered.contains("does not support field or index projections yet"));
    assert!(!rendered.contains("does not support assignment to field or index projections yet"));
}

#[test]
fn emits_async_main_entry_lifecycle_with_aliased_guard_refined_const_backed_projected_root_dynamic_task_handle_reinit_in_program_mode()
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

const INDEX: Int = 0

async fn worker(value: Int) -> Int {
return value
}

async fn main() -> Int {
var pending = Pending {
    tasks: [worker(8), worker(13)],
}
let alias = pending.tasks
let slot = Slot { value: INDEX }
if slot.value == 0 {
    let first = await alias[slot.value]
    pending.tasks[0] = worker(first + 4)
}
let second = await alias[0]
let tail = await pending.tasks[1]
return second + tail
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
    assert!(
        rendered
            .matches("getelementptr inbounds { [2 x ptr] }, ptr")
            .count()
            >= 4
    );
    assert!(
        rendered
            .matches("getelementptr inbounds [2 x ptr], ptr")
            .count()
            >= 4
    );
    assert!(
        rendered
            .matches("getelementptr inbounds { i64 }, ptr")
            .count()
            >= 2
    );
    assert!(rendered.matches("store ptr").count() >= 5);
    assert!(!rendered.contains("does not support field or index projections yet"));
    assert!(!rendered.contains("does not support assignment to field or index projections yet"));
}

#[test]
fn emits_async_main_entry_lifecycle_with_aliased_projected_root_task_handle_tuple_repackage_reinit_in_program_mode()
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

async fn worker(value: Int) -> Int {
return value
}

async fn main() -> Int {
var pending = Pending {
    tasks: [worker(9), worker(14)],
}
let slot = Slot { value: 0 }
let alias = pending.tasks
let first = await alias[slot.value]
pending.tasks[slot.value] = worker(first + 3)
let pair = (alias[slot.value], worker(5))
let second = await pair[0]
let extra = await pair[1]
let tail = await pending.tasks[1]
return second + extra + tail
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.contains("define i32 @main()"));
    assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
    assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
    assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
    assert!(rendered.matches("@qlrt_task_await").count() >= 5);
    assert!(
        rendered
            .matches("getelementptr inbounds { [2 x ptr] }, ptr")
            .count()
            >= 4
    );
    assert!(
        rendered
            .matches("getelementptr inbounds [2 x ptr], ptr")
            .count()
            >= 4
    );
    assert!(
        rendered
            .matches("getelementptr inbounds { ptr, ptr }, ptr")
            .count()
            >= 2
    );
    assert!(
        rendered
            .matches("getelementptr inbounds { i64 }, ptr")
            .count()
            >= 2
    );
    assert!(rendered.matches("store ptr").count() >= 6);
    assert!(!rendered.contains("does not support field or index projections yet"));
    assert!(!rendered.contains("does not support assignment to field or index projections yet"));
}

#[test]
fn emits_async_main_entry_lifecycle_with_aliased_projected_root_task_handle_struct_repackage_reinit_in_program_mode()
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
left: Task[Int],
right: Task[Int],
}

async fn worker(value: Int) -> Int {
return value
}

async fn main() -> Int {
var pending = Pending {
    tasks: [worker(9), worker(14)],
}
let slot = Slot { value: 0 }
let alias = pending.tasks
let first = await alias[slot.value]
pending.tasks[slot.value] = worker(first + 3)
let bundle = Bundle {
    left: alias[slot.value],
    right: worker(6),
}
let second = await bundle.left
let extra = await bundle.right
let tail = await pending.tasks[1]
return second + extra + tail
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.contains("define i32 @main()"));
    assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
    assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
    assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
    assert!(rendered.matches("@qlrt_task_await").count() >= 5);
    assert!(
        rendered
            .matches("getelementptr inbounds { [2 x ptr] }, ptr")
            .count()
            >= 4
    );
    assert!(
        rendered
            .matches("getelementptr inbounds [2 x ptr], ptr")
            .count()
            >= 4
    );
    assert!(
        rendered
            .matches("getelementptr inbounds { ptr, ptr }, ptr")
            .count()
            >= 2
    );
    assert!(rendered.matches("store ptr").count() >= 6);
    assert!(!rendered.contains("does not support field or index projections yet"));
    assert!(!rendered.contains("does not support assignment to field or index projections yet"));
}

#[test]
fn emits_async_main_entry_lifecycle_with_aliased_projected_root_task_handle_nested_repackage_reinit_in_program_mode()
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
left: Task[Int],
right: Task[Int],
}

struct Envelope {
bundle: Bundle,
tail: Task[Int],
}

async fn worker(value: Int) -> Int {
return value
}

async fn main() -> Int {
var pending = Pending {
    tasks: [worker(9), worker(14)],
}
let slot = Slot { value: 0 }
let alias = pending.tasks
let first = await alias[slot.value]
pending.tasks[slot.value] = worker(first + 3)
let env = Envelope {
    bundle: Bundle {
        left: alias[slot.value],
        right: worker(7),
    },
    tail: pending.tasks[1],
}
let second = await env.bundle.left
let extra = await env.bundle.right
let tail = await env.tail
return second + extra + tail
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.contains("define i32 @main()"));
    assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
    assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
    assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
    assert!(rendered.matches("@qlrt_task_await").count() >= 5);
    assert!(
        rendered
            .matches("getelementptr inbounds { [2 x ptr] }, ptr")
            .count()
            >= 4
    );
    assert!(
        rendered
            .matches("getelementptr inbounds [2 x ptr], ptr")
            .count()
            >= 4
    );
    assert!(
        rendered
            .matches("getelementptr inbounds { { ptr, ptr }, ptr }, ptr")
            .count()
            >= 2
    );
    assert!(rendered.matches("store ptr").count() >= 7);
    assert!(!rendered.contains("does not support field or index projections yet"));
    assert!(!rendered.contains("does not support assignment to field or index projections yet"));
}

#[test]
fn emits_async_main_entry_lifecycle_with_aliased_projected_root_task_handle_nested_repackage_spawn_in_program_mode()
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
left: Task[Int],
right: Task[Int],
}

struct Envelope {
bundle: Bundle,
tail: Task[Int],
}

async fn worker(value: Int) -> Int {
return value
}

async fn main() -> Int {
var pending = Pending {
    tasks: [worker(9), worker(14)],
}
let slot = Slot { value: 0 }
let alias = pending.tasks
let first = await alias[slot.value]
pending.tasks[slot.value] = worker(first + 4)
let env = Envelope {
    bundle: Bundle {
        left: alias[slot.value],
        right: worker(7),
    },
    tail: pending.tasks[1],
}
let running = spawn env.bundle.left
let second = await running
let extra = await env.bundle.right
let tail = await env.tail
return second + extra + tail
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
    assert!(rendered.matches("@qlrt_task_await").count() >= 5);
    assert!(
        rendered
            .matches("getelementptr inbounds { { ptr, ptr }, ptr }, ptr")
            .count()
            >= 2
    );
    assert!(
        rendered
            .matches("getelementptr inbounds { ptr, ptr }, ptr")
            .count()
            >= 2
    );
    assert!(rendered.matches("store ptr").count() >= 7);
    assert!(!rendered.contains("does not support field or index projections yet"));
    assert!(!rendered.contains("does not support assignment to field or index projections yet"));
}

#[test]
fn emits_async_main_entry_lifecycle_with_aliased_projected_root_task_handle_array_repackage_spawn_in_program_mode()
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

async fn worker(value: Int) -> Int {
return value
}

async fn main() -> Int {
var pending = Pending {
    tasks: [worker(9), worker(14)],
}
let slot = Slot { value: 0 }
let alias = pending.tasks
let first = await alias[slot.value]
pending.tasks[slot.value] = worker(first + 4)
let tasks = [alias[slot.value], worker(10)]
let running = spawn tasks[0]
let second = await running
let extra = await tasks[1]
let tail = await pending.tasks[1]
return second + extra + tail
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
    assert!(rendered.matches("@qlrt_task_await").count() >= 5);
    assert!(
        rendered
            .matches("getelementptr inbounds [2 x ptr], ptr")
            .count()
            >= 4
    );
    assert!(
        rendered
            .matches("getelementptr inbounds { [2 x ptr] }, ptr")
            .count()
            >= 2
    );
    assert!(rendered.matches("store ptr").count() >= 7);
    assert!(!rendered.contains("does not support field or index projections yet"));
    assert!(!rendered.contains("does not support assignment to field or index projections yet"));
}

#[test]
fn emits_async_main_entry_lifecycle_with_aliased_guard_refined_const_backed_projected_root_task_handle_nested_repackage_reinit_in_program_mode()
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
left: Task[Int],
right: Task[Int],
}

struct Envelope {
bundle: Bundle,
tail: Task[Int],
}

const INDEX: Int = 0

async fn worker(value: Int) -> Int {
return value
}

async fn main() -> Int {
var pending = Pending {
    tasks: [worker(8), worker(14)],
}
let alias = pending.tasks
let slot = Slot { value: INDEX }
if slot.value == 0 {
    let first = await alias[slot.value]
    pending.tasks[0] = worker(first + 5)
}
let env = Envelope {
    bundle: Bundle {
        left: alias[slot.value],
        right: worker(9),
    },
    tail: pending.tasks[1],
}
let second = await env.bundle.left
let extra = await env.bundle.right
let tail = await env.tail
return second + extra + tail
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.contains("define i32 @main()"));
    assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
    assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
    assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
    assert!(rendered.matches("@qlrt_task_await").count() >= 5);
    assert!(
        rendered
            .matches("getelementptr inbounds { [2 x ptr] }, ptr")
            .count()
            >= 4
    );
    assert!(
        rendered
            .matches("getelementptr inbounds [2 x ptr], ptr")
            .count()
            >= 4
    );
    assert!(
        rendered
            .matches("getelementptr inbounds { { ptr, ptr }, ptr }, ptr")
            .count()
            >= 2
    );
    assert!(
        rendered
            .matches("getelementptr inbounds { i64 }, ptr")
            .count()
            >= 2
    );
    assert!(rendered.matches("store ptr").count() >= 7);
    assert!(!rendered.contains("does not support field or index projections yet"));
    assert!(!rendered.contains("does not support assignment to field or index projections yet"));
}

#[test]
fn emits_async_main_entry_lifecycle_with_aliased_guard_refined_const_backed_projected_root_task_handle_nested_repackage_spawn_in_program_mode()
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
left: Task[Int],
right: Task[Int],
}

struct Envelope {
bundle: Bundle,
tail: Task[Int],
}

const INDEX: Int = 0

async fn worker(value: Int) -> Int {
return value
}

async fn main() -> Int {
var pending = Pending {
    tasks: [worker(8), worker(14)],
}
let alias = pending.tasks
let slot = Slot { value: INDEX }
if slot.value == 0 {
    let first = await alias[slot.value]
    pending.tasks[0] = worker(first + 5)
}
let env = Envelope {
    bundle: Bundle {
        left: alias[slot.value],
        right: worker(11),
    },
    tail: pending.tasks[1],
}
let running = spawn env.bundle.left
let second = await running
let extra = await env.bundle.right
let tail = await env.tail
return second + extra + tail
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
    assert!(rendered.matches("@qlrt_task_await").count() >= 5);
    assert!(
        rendered
            .matches("getelementptr inbounds { { ptr, ptr }, ptr }, ptr")
            .count()
            >= 2
    );
    assert!(
        rendered
            .matches("getelementptr inbounds { i64 }, ptr")
            .count()
            >= 2
    );
    assert!(rendered.matches("store ptr").count() >= 7);
    assert!(!rendered.contains("does not support field or index projections yet"));
    assert!(!rendered.contains("does not support assignment to field or index projections yet"));
}

#[test]
fn emits_async_main_entry_lifecycle_with_aliased_guard_refined_const_backed_projected_root_task_handle_array_repackage_spawn_in_program_mode()
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

const INDEX: Int = 0

async fn worker(value: Int) -> Int {
return value
}

async fn main() -> Int {
var pending = Pending {
    tasks: [worker(8), worker(14)],
}
let alias = pending.tasks
let slot = Slot { value: INDEX }
if slot.value == 0 {
    let first = await alias[slot.value]
    pending.tasks[0] = worker(first + 5)
}
let tasks = [alias[slot.value], worker(13)]
let running = spawn tasks[0]
let second = await running
let extra = await tasks[1]
let tail = await pending.tasks[1]
return second + extra + tail
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
    assert!(rendered.matches("@qlrt_task_await").count() >= 5);
    assert!(
        rendered
            .matches("getelementptr inbounds [2 x ptr], ptr")
            .count()
            >= 4
    );
    assert!(
        rendered
            .matches("getelementptr inbounds { [2 x ptr] }, ptr")
            .count()
            >= 2
    );
    assert!(rendered.matches("store ptr").count() >= 7);
    assert!(!rendered.contains("does not support field or index projections yet"));
    assert!(!rendered.contains("does not support assignment to field or index projections yet"));
}

#[test]
fn emits_async_main_entry_lifecycle_with_aliased_projected_root_task_handle_nested_array_repackage_spawn_in_program_mode()
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

async fn worker(value: Int) -> Int {
return value
}

async fn main() -> Int {
var pending = Pending {
    tasks: [worker(9), worker(14)],
}
let slot = Slot { value: 0 }
let alias = pending.tasks
let first = await alias[slot.value]
pending.tasks[slot.value] = worker(first + 6)
let env = Envelope {
    bundle: Bundle {
        tasks: [alias[slot.value], worker(12)],
    },
    tail: pending.tasks[1],
}
let running = spawn env.bundle.tasks[0]
let second = await running
let extra = await env.bundle.tasks[1]
let tail = await env.tail
return second + extra + tail
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
    assert!(rendered.matches("@qlrt_task_await").count() >= 5);
    assert!(
        rendered
            .matches("getelementptr inbounds { { [2 x ptr] }, ptr }, ptr")
            .count()
            >= 2
    );
    assert!(
        rendered
            .matches("getelementptr inbounds { [2 x ptr] }, ptr")
            .count()
            >= 2
    );
    assert!(
        rendered
            .matches("getelementptr inbounds [2 x ptr], ptr")
            .count()
            >= 4
    );
    assert!(rendered.matches("store ptr").count() >= 7);
    assert!(!rendered.contains("does not support field or index projections yet"));
    assert!(!rendered.contains("does not support assignment to field or index projections yet"));
}

#[test]
fn emits_async_main_entry_lifecycle_with_aliased_guard_refined_const_backed_projected_root_task_handle_nested_array_repackage_spawn_in_program_mode()
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

async fn main() -> Int {
var pending = Pending {
    tasks: [worker(8), worker(14)],
}
let alias = pending.tasks
let slot = Slot { value: INDEX }
if slot.value == 0 {
    let first = await alias[slot.value]
    pending.tasks[0] = worker(first + 7)
}
let env = Envelope {
    bundle: Bundle {
        tasks: [alias[slot.value], worker(17)],
    },
    tail: pending.tasks[1],
}
let running = spawn env.bundle.tasks[0]
let second = await running
let extra = await env.bundle.tasks[1]
let tail = await env.tail
return second + extra + tail
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
    assert!(rendered.matches("@qlrt_task_await").count() >= 5);
    assert!(
        rendered
            .matches("getelementptr inbounds { { [2 x ptr] }, ptr }, ptr")
            .count()
            >= 2
    );
    assert!(
        rendered
            .matches("getelementptr inbounds { [2 x ptr] }, ptr")
            .count()
            >= 3
    );
    assert!(
        rendered
            .matches("getelementptr inbounds [2 x ptr], ptr")
            .count()
            >= 4
    );
    assert!(
        rendered
            .matches("getelementptr inbounds { i64 }, ptr")
            .count()
            >= 2
    );
    assert!(rendered.matches("store ptr").count() >= 7);
    assert!(!rendered.contains("does not support field or index projections yet"));
    assert!(!rendered.contains("does not support assignment to field or index projections yet"));
}

#[test]
fn emits_async_main_entry_lifecycle_with_aliased_projected_root_task_handle_forwarded_nested_array_repackage_spawn_in_program_mode()
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

async fn worker(value: Int) -> Int {
return value
}

fn forward(task: Task[Int]) -> Task[Int] {
return task
}

async fn main() -> Int {
var pending = Pending {
    tasks: [worker(9), worker(14)],
}
let slot = Slot { value: 0 }
let alias = pending.tasks
let first = await alias[slot.value]
pending.tasks[slot.value] = worker(first + 8)
let env = Envelope {
    bundle: Bundle {
        tasks: [forward(alias[slot.value]), worker(21)],
    },
    tail: pending.tasks[1],
}
let running = spawn env.bundle.tasks[0]
let second = await running
let extra = await env.bundle.tasks[1]
let tail = await env.tail
return second + extra + tail
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
    assert!(rendered.matches("@qlrt_task_await").count() >= 5);
    assert!(rendered.matches("_forward(").count() >= 2);
    assert!(
        rendered
            .matches("getelementptr inbounds { { [2 x ptr] }, ptr }, ptr")
            .count()
            >= 2
    );
    assert!(
        rendered
            .matches("getelementptr inbounds { [2 x ptr] }, ptr")
            .count()
            >= 2
    );
    assert!(
        rendered
            .matches("getelementptr inbounds [2 x ptr], ptr")
            .count()
            >= 4
    );
    assert!(rendered.matches("store ptr").count() >= 7);
    assert!(!rendered.contains("does not support field or index projections yet"));
    assert!(!rendered.contains("does not support assignment to field or index projections yet"));
}
