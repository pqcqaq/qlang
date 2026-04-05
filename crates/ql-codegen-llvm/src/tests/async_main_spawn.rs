use super::*;

#[test]
fn emits_async_main_entry_lifecycle_with_aliased_guard_refined_const_backed_projected_root_task_handle_forwarded_nested_array_repackage_spawn_in_program_mode()
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
var pending = Pending {
    tasks: [worker(8), worker(14)],
}
let alias = pending.tasks
let slot = Slot { value: INDEX }
if slot.value == 0 {
    let first = await alias[slot.value]
    pending.tasks[0] = worker(first + 9)
}
let env = Envelope {
    bundle: Bundle {
        tasks: [forward(alias[slot.value]), worker(23)],
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
fn emits_async_main_entry_lifecycle_with_composed_dynamic_task_handle_nested_array_repackage_spawn_in_program_mode()
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

fn choose() -> Int {
return 0
}

async fn main() -> Int {
let row = choose()
let slots = [row, row]
var pending = Pending {
    tasks: [worker(9), worker(14)],
}
let alias = pending.tasks
let first = await alias[slots[row]]
pending.tasks[slots[row]] = worker(first + 6)
let env = Envelope {
    bundle: Bundle {
        tasks: [alias[slots[row]], worker(18)],
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
            .matches("getelementptr inbounds [2 x i64], ptr")
            .count()
            >= 3
    );
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
fn emits_async_main_entry_lifecycle_with_alias_sourced_composed_dynamic_task_handle_nested_array_repackage_spawn_in_program_mode()
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

fn choose() -> Int {
return 0
}

async fn main() -> Int {
let row = choose()
let slots = [row, row]
let alias_slots = slots
var pending = Pending {
    tasks: [worker(9), worker(14)],
}
let alias = pending.tasks
let first = await alias[alias_slots[row]]
pending.tasks[slots[row]] = worker(first + 6)
let env = Envelope {
    bundle: Bundle {
        tasks: [alias[alias_slots[row]], worker(19)],
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
            .matches("getelementptr inbounds [2 x i64], ptr")
            .count()
            >= 3
    );
    assert!(rendered.matches("store [2 x i64]").count() >= 2);
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
fn emits_async_main_entry_lifecycle_with_guarded_alias_sourced_composed_dynamic_task_handle_nested_array_repackage_spawn_in_program_mode()
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

fn choose() -> Int {
return INDEX
}

async fn main() -> Int {
let row = choose()
let slots = [row, row]
let alias_slots = slots
var pending = Pending {
    tasks: [worker(8), worker(14)],
}
let alias = pending.tasks
let slot = Slot { value: INDEX }
if slot.value == 0 {
    let first = await alias[alias_slots[row]]
    pending.tasks[slots[row]] = worker(first + 8)
}
let env = Envelope {
    bundle: Bundle {
        tasks: [alias[alias_slots[row]], worker(20)],
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
            .matches("getelementptr inbounds [2 x i64], ptr")
            .count()
            >= 3
    );
    assert!(rendered.matches("store [2 x i64]").count() >= 2);
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
fn emits_async_main_entry_lifecycle_with_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn_with_tail_field_in_program_mode()
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
tail: Task[Int],
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

fn choose() -> Int {
return 0
}

fn forward(task: Task[Int]) -> Task[Int] {
return task
}

async fn main() -> Int {
let row = choose()
let slots = [row, row]
let alias_slots = slots
var pending = Pending {
    tasks: [worker(9), worker(14)],
    tail: worker(17),
}
let alias = pending.tasks
let first = await alias[alias_slots[row]]
pending.tasks[slots[row]] = worker(first + 9)
let env = Envelope {
    bundle: Bundle {
        tasks: [forward(alias[alias_slots[row]]), worker(24)],
    },
    tail: pending.tail,
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
            .matches("getelementptr inbounds [2 x i64], ptr")
            .count()
            >= 3
    );
    assert!(
        rendered
            .matches("getelementptr inbounds [2 x ptr], ptr")
            .count()
            >= 4
    );
    assert!(rendered.matches("store ptr").count() >= 8);
    assert!(!rendered.contains("does not support field or index projections yet"));
    assert!(!rendered.contains("does not support assignment to field or index projections yet"));
}

#[test]
fn emits_async_main_entry_lifecycle_with_guarded_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn_with_tail_field_in_program_mode()
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
tail: Task[Int],
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

fn choose() -> Int {
return INDEX
}

fn forward(task: Task[Int]) -> Task[Int] {
return task
}

async fn main() -> Int {
let row = choose()
let slots = [row, row]
let alias_slots = slots
var pending = Pending {
    tasks: [worker(8), worker(14)],
    tail: worker(19),
}
let alias = pending.tasks
let slot = Slot { value: INDEX }
if slot.value == 0 {
    let first = await alias[alias_slots[row]]
    pending.tasks[slots[row]] = worker(first + 10)
}
let env = Envelope {
    bundle: Bundle {
        tasks: [forward(alias[alias_slots[row]]), worker(26)],
    },
    tail: pending.tail,
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
            .matches("getelementptr inbounds [2 x i64], ptr")
            .count()
            >= 3
    );
    assert!(
        rendered
            .matches("getelementptr inbounds { i64 }, ptr")
            .count()
            >= 2
    );
    assert!(rendered.matches("store ptr").count() >= 8);
    assert!(!rendered.contains("does not support field or index projections yet"));
    assert!(!rendered.contains("does not support assignment to field or index projections yet"));
}

#[test]
fn emits_async_main_entry_lifecycle_with_const_backed_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn_in_program_mode()
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
let row = INDEX
let slots = [row, row]
let alias_slots = slots
var pending = Pending {
    tasks: [worker(9), worker(14)],
}
let alias = pending.tasks
let first = await alias[alias_slots[row]]
pending.tasks[slots[row]] = worker(first + 11)
let env = Envelope {
    bundle: Bundle {
        tasks: [forward(alias[alias_slots[row]]), worker(27)],
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
            .matches("getelementptr inbounds [2 x i64], ptr")
            .count()
            >= 3
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
fn emits_async_main_entry_lifecycle_with_guarded_const_backed_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn_in_program_mode()
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
let row = INDEX
let slots = [row, row]
let alias_slots = slots
var pending = Pending {
    tasks: [worker(8), worker(14)],
}
let alias = pending.tasks
let slot = Slot { value: INDEX }
if slot.value == 0 {
    let first = await alias[alias_slots[row]]
    pending.tasks[slots[row]] = worker(first + 12)
}
let env = Envelope {
    bundle: Bundle {
        tasks: [forward(alias[alias_slots[row]]), worker(28)],
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
            .matches("getelementptr inbounds [2 x i64], ptr")
            .count()
            >= 3
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
fn emits_async_main_entry_lifecycle_with_guarded_const_backed_double_root_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn_in_program_mode()
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
let row = INDEX
let slots = [row, row]
let alias_slots = slots
var pending = Pending {
    tasks: [worker(8), worker(14)],
}
let root = pending.tasks
let alias = root
let slot = Slot { value: INDEX }
if slot.value == 0 {
    let first = await alias[alias_slots[row]]
    pending.tasks[slots[row]] = worker(first + 13)
}
let env = Envelope {
    bundle: Bundle {
        tasks: [forward(alias[alias_slots[row]]), worker(29)],
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
            .matches("getelementptr inbounds [2 x i64], ptr")
            .count()
            >= 3
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
fn emits_async_main_entry_lifecycle_with_guarded_const_backed_double_root_double_source_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn_in_program_mode()
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
let row = INDEX
let slots = [row, row]
let slot_root = slots
let alias_slots = slot_root
var pending = Pending {
    tasks: [worker(8), worker(14)],
}
let root = pending.tasks
let alias = root
let slot = Slot { value: INDEX }
if slot.value == 0 {
    let first = await alias[alias_slots[row]]
    pending.tasks[slots[row]] = worker(first + 14)
}
let env = Envelope {
    bundle: Bundle {
        tasks: [forward(alias[alias_slots[row]]), worker(30)],
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
            .matches("getelementptr inbounds [2 x i64], ptr")
            .count()
            >= 3
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
fn emits_async_main_entry_lifecycle_with_guarded_const_backed_double_root_double_source_row_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn_in_program_mode()
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
let alias_slots = slot_root
var pending = Pending {
    tasks: [worker(8), worker(14)],
}
let root = pending.tasks
let alias = root
let slot = Slot { value: INDEX }
if slot.value == 0 {
    let first = await alias[alias_slots[row]]
    pending.tasks[slots[row]] = worker(first + 15)
}
let env = Envelope {
    bundle: Bundle {
        tasks: [forward(alias[alias_slots[row]]), worker(31)],
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
            .matches("getelementptr inbounds [2 x i64], ptr")
            .count()
            >= 3
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
fn emits_async_main_entry_lifecycle_with_guarded_const_backed_double_root_double_source_row_slot_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn_in_program_mode()
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
let alias_slots = slot_root
var pending = Pending {
    tasks: [worker(8), worker(14)],
}
let root = pending.tasks
let alias = root
let slot = Slot { value: INDEX }
let slot_alias = slot
if slot_alias.value == 0 {
    let first = await alias[alias_slots[row]]
    pending.tasks[slots[row]] = worker(first + 16)
}
let env = Envelope {
    bundle: Bundle {
        tasks: [forward(alias[alias_slots[row]]), worker(34)],
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
            .matches("getelementptr inbounds [2 x i64], ptr")
            .count()
            >= 3
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
fn emits_async_main_entry_lifecycle_with_guarded_const_backed_triple_root_double_source_row_slot_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn_in_program_mode()
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
let alias_slots = slot_root
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
    pending.tasks[slots[row]] = worker(first + 17)
}
let env = Envelope {
    bundle: Bundle {
        tasks: [forward(alias[alias_slots[row]]), worker(35)],
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
            .matches("getelementptr inbounds [2 x i64], ptr")
            .count()
            >= 3
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
fn emits_async_main_entry_lifecycle_with_guarded_const_backed_triple_root_triple_source_row_slot_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn_in_program_mode()
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
    pending.tasks[slots[row]] = worker(first + 18)
}
let env = Envelope {
    bundle: Bundle {
        tasks: [forward(alias[alias_slots[row]]), worker(36)],
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
            .matches("getelementptr inbounds [2 x i64], ptr")
            .count()
            >= 3
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
fn emits_async_main_entry_lifecycle_with_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn_in_program_mode()
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
    pending.tasks[slots[row]] = worker(first + 19)
}
let tail_tasks = pending.tasks
let env = Envelope {
    bundle: Bundle {
        tasks: [forward(alias[alias_slots[row]]), worker(37)],
    },
    tail: tail_tasks[1],
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
            .matches("getelementptr inbounds [2 x i64], ptr")
            .count()
            >= 3
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
fn emits_async_main_entry_lifecycle_with_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_task_handle_forwarded_alias_nested_array_repackage_spawn_in_program_mode()
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
    pending.tasks[slots[row]] = worker(first + 20)
}
let tail_tasks = pending.tasks
let forwarded = forward(alias[alias_slots[row]])
let running_task = forwarded
let env = Envelope {
    bundle: Bundle {
        tasks: [running_task, worker(38)],
    },
    tail: tail_tasks[1],
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
            .matches("getelementptr inbounds [2 x i64], ptr")
            .count()
            >= 3
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
fn emits_async_main_entry_lifecycle_with_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_task_handle_queued_spawn_in_program_mode()
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
    pending.tasks[slots[row]] = worker(first + 21)
}
let tail_tasks = pending.tasks
let forwarded = forward(alias[alias_slots[row]])
let running_task = forwarded
let env = Envelope {
    bundle: Bundle {
        tasks: [running_task, worker(39)],
    },
    tail: tail_tasks[1],
}
let queued = env.bundle.tasks[0]
let running = spawn queued
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
            .matches("getelementptr inbounds [2 x i64], ptr")
            .count()
            >= 3
    );
    assert!(
        rendered
            .matches("getelementptr inbounds { i64 }, ptr")
            .count()
            >= 3
    );
    assert!(rendered.matches("store ptr").count() >= 8);
    assert!(!rendered.contains("does not support field or index projections yet"));
    assert!(!rendered.contains("does not support assignment to field or index projections yet"));
}

#[test]
fn emits_async_main_entry_lifecycle_with_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_task_handle_queued_root_spawn_in_program_mode()
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
    pending.tasks[slots[row]] = worker(first + 22)
}
let tail_tasks = pending.tasks
let forwarded = forward(alias[alias_slots[row]])
let running_task = forwarded
let env = Envelope {
    bundle: Bundle {
        tasks: [running_task, worker(40)],
    },
    tail: tail_tasks[1],
}
let queued_tasks = env.bundle.tasks
let queued = queued_tasks[0]
let running = spawn queued
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
            .matches("getelementptr inbounds [2 x i64], ptr")
            .count()
            >= 3
    );
    assert!(
        rendered
            .matches("getelementptr inbounds { i64 }, ptr")
            .count()
            >= 3
    );
    assert!(rendered.matches("store ptr").count() >= 8);
    assert!(!rendered.contains("does not support field or index projections yet"));
    assert!(!rendered.contains("does not support assignment to field or index projections yet"));
}

#[test]
fn emits_async_main_entry_lifecycle_with_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_task_handle_queued_root_alias_spawn_in_program_mode()
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
    pending.tasks[slots[row]] = worker(first + 23)
}
let tail_tasks = pending.tasks
let forwarded = forward(alias[alias_slots[row]])
let running_task = forwarded
let env = Envelope {
    bundle: Bundle {
        tasks: [running_task, worker(41)],
    },
    tail: tail_tasks[1],
}
let queue_root = env.bundle.tasks
let queued_tasks = queue_root
let queued = queued_tasks[0]
let running = spawn queued
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
            .matches("getelementptr inbounds [2 x i64], ptr")
            .count()
            >= 3
    );
    assert!(
        rendered
            .matches("getelementptr inbounds { i64 }, ptr")
            .count()
            >= 2
    );
    assert!(rendered.matches("store ptr").count() >= 9);
    assert!(!rendered.contains("does not support field or index projections yet"));
    assert!(!rendered.contains("does not support assignment to field or index projections yet"));
}

#[test]
fn emits_async_main_entry_lifecycle_with_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_task_handle_queued_root_chain_spawn_in_program_mode()
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
    pending.tasks[slots[row]] = worker(first + 24)
}
let tail_tasks = pending.tasks
let forwarded = forward(alias[alias_slots[row]])
let running_task = forwarded
let env = Envelope {
    bundle: Bundle {
        tasks: [running_task, worker(42)],
    },
    tail: tail_tasks[1],
}
let queue_root = env.bundle.tasks
let queue_alias_root = queue_root
let queued_tasks = queue_alias_root
let queued = queued_tasks[0]
let running = spawn queued
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
            .matches("getelementptr inbounds [2 x i64], ptr")
            .count()
            >= 3
    );
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
fn emits_async_main_entry_lifecycle_with_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_task_handle_queued_local_alias_spawn_in_program_mode()
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
    pending.tasks[slots[row]] = worker(first + 25)
}
let tail_tasks = pending.tasks
let forwarded = forward(alias[alias_slots[row]])
let running_task = forwarded
let env = Envelope {
    bundle: Bundle {
        tasks: [running_task, worker(43)],
    },
    tail: tail_tasks[1],
}
let queue_root = env.bundle.tasks
let queue_alias_root = queue_root
let queued_tasks = queue_alias_root
let queued = queued_tasks[0]
let queued_alias = queued
let running = spawn queued_alias
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
            .matches("getelementptr inbounds [2 x i64], ptr")
            .count()
            >= 3
    );
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
fn emits_async_main_entry_lifecycle_with_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_task_handle_queued_local_chain_spawn_in_program_mode()
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
    pending.tasks[slots[row]] = worker(first + 26)
}
let tail_tasks = pending.tasks
let forwarded = forward(alias[alias_slots[row]])
let running_task = forwarded
let env = Envelope {
    bundle: Bundle {
        tasks: [running_task, worker(44)],
    },
    tail: tail_tasks[1],
}
let queue_root = env.bundle.tasks
let queue_alias_root = queue_root
let queued_tasks = queue_alias_root
let queued = queued_tasks[0]
let queued_alias = queued
let queued_final = queued_alias
let running = spawn queued_final
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
            .matches("getelementptr inbounds [2 x i64], ptr")
            .count()
            >= 4
    );
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
fn emits_async_main_entry_lifecycle_with_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_task_handle_queued_local_forward_spawn_in_program_mode()
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
    pending.tasks[slots[row]] = worker(first + 27)
}
let tail_tasks = pending.tasks
let forwarded = forward(alias[alias_slots[row]])
let running_task = forwarded
let env = Envelope {
    bundle: Bundle {
        tasks: [running_task, worker(45)],
    },
    tail: tail_tasks[1],
}
let queue_root = env.bundle.tasks
let queue_alias_root = queue_root
let queued_tasks = queue_alias_root
let queued = queued_tasks[0]
let queued_alias = queued
let queued_final = queued_alias
let queued_ready = forward(queued_final)
let running = spawn queued_ready
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
    assert!(rendered.matches("_forward(").count() >= 3);
    assert!(
        rendered
            .matches("getelementptr inbounds [2 x i64], ptr")
            .count()
            >= 3
    );
    assert!(
        rendered
            .matches("getelementptr inbounds { i64 }, ptr")
            .count()
            >= 3
    );
    assert!(rendered.matches("store ptr").count() >= 10);
    assert!(!rendered.contains("does not support field or index projections yet"));
    assert!(!rendered.contains("does not support assignment to field or index projections yet"));
}

#[test]
fn emits_async_main_entry_lifecycle_with_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_task_handle_queued_local_inline_forward_spawn_in_program_mode()
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
    pending.tasks[slots[row]] = worker(first + 28)
}
let tail_tasks = pending.tasks
let forwarded = forward(alias[alias_slots[row]])
let running_task = forwarded
let env = Envelope {
    bundle: Bundle {
        tasks: [running_task, worker(46)],
    },
    tail: tail_tasks[1],
}
let queue_root = env.bundle.tasks
let queue_alias_root = queue_root
let queued_tasks = queue_alias_root
let queued = queued_tasks[0]
let queued_alias = queued
let queued_final = queued_alias
let running = spawn forward(queued_final)
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
    assert!(rendered.matches("_forward(").count() >= 3);
    assert!(
        rendered
            .matches("getelementptr inbounds [2 x i64], ptr")
            .count()
            >= 3
    );
    assert!(
        rendered
            .matches("getelementptr inbounds { i64 }, ptr")
            .count()
            >= 3
    );
    assert!(rendered.matches("store ptr").count() >= 10);
    assert!(!rendered.contains("does not support field or index projections yet"));
    assert!(!rendered.contains("does not support assignment to field or index projections yet"));
}

#[test]
fn emits_async_main_entry_lifecycle_with_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_task_handle_bundle_inline_forward_spawn_in_program_mode()
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
    pending.tasks[slots[row]] = worker(first + 29)
}
let tail_tasks = pending.tasks
let forwarded = forward(alias[alias_slots[row]])
let running_task = forwarded
let env = Envelope {
    bundle: Bundle {
        tasks: [running_task, worker(47)],
    },
    tail: tail_tasks[1],
}
let running = spawn forward(env.bundle.tasks[0])
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
    assert!(rendered.matches("_forward(").count() >= 3);
    assert!(
        rendered
            .matches("getelementptr inbounds [2 x i64], ptr")
            .count()
            >= 3
    );
    assert!(
        rendered
            .matches("getelementptr inbounds { i64 }, ptr")
            .count()
            >= 3
    );
    assert!(rendered.matches("store ptr").count() >= 10);
    assert!(!rendered.contains("does not support field or index projections yet"));
    assert!(!rendered.contains("does not support assignment to field or index projections yet"));
}

#[test]
fn emits_async_main_entry_lifecycle_with_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_task_handle_bundle_slot_inline_forward_spawn_in_program_mode()
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
    pending.tasks[slots[row]] = worker(first + 30)
}
let tail_tasks = pending.tasks
let forwarded = forward(alias[alias_slots[row]])
let running_task = forwarded
let env = Envelope {
    bundle: Bundle {
        tasks: [running_task, worker(48)],
    },
    tail: tail_tasks[1],
}
let bundle_slot = Slot { value: INDEX }
let bundle_slot_alias = bundle_slot
let running = spawn forward(env.bundle.tasks[bundle_slot_alias.value])
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
    assert!(rendered.matches("_forward(").count() >= 3);
    assert!(
        rendered
            .matches("getelementptr inbounds [2 x i64], ptr")
            .count()
            >= 3
    );
    assert!(
        rendered
            .matches("getelementptr inbounds { i64 }, ptr")
            .count()
            >= 3
    );
    assert!(rendered.matches("store ptr").count() >= 10);
    assert!(!rendered.contains("does not support field or index projections yet"));
    assert!(!rendered.contains("does not support assignment to field or index projections yet"));
}

#[test]
fn emits_async_main_entry_lifecycle_with_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_task_handle_bundle_forward_spawn_in_program_mode()
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
    pending.tasks[slots[row]] = worker(first + 41)
}
let tail_tasks = pending.tasks
let forwarded = forward(alias[alias_slots[row]])
let running_task = forwarded
let env = Envelope {
    bundle: Bundle {
        tasks: [running_task, worker(73)],
    },
    tail: tail_tasks[1],
}
let bundle_tasks = env.bundle.tasks
let bundled = bundle_tasks[0]
let bundle_ready = forward(bundled)
let running = spawn bundle_ready
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
    assert!(rendered.matches("_forward(").count() >= 3);
    assert!(
        rendered
            .matches("getelementptr inbounds [2 x i64], ptr")
            .count()
            >= 3
    );
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
fn emits_async_main_entry_lifecycle_with_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_task_handle_bundle_alias_forward_spawn_in_program_mode()
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
    pending.tasks[slots[row]] = worker(first + 42)
}
let tail_tasks = pending.tasks
let forwarded = forward(alias[alias_slots[row]])
let running_task = forwarded
let env = Envelope {
    bundle: Bundle {
        tasks: [running_task, worker(74)],
    },
    tail: tail_tasks[1],
}
let bundle_root = env.bundle.tasks
let bundle_tasks = bundle_root
let bundled = bundle_tasks[0]
let bundle_ready = forward(bundled)
let running = spawn bundle_ready
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
    assert!(rendered.matches("_forward(").count() >= 3);
    assert!(
        rendered
            .matches("getelementptr inbounds [2 x i64], ptr")
            .count()
            >= 3
    );
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
fn emits_async_main_entry_lifecycle_with_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_task_handle_bundle_chain_forward_spawn_in_program_mode()
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
    pending.tasks[slots[row]] = worker(first + 43)
}
let tail_tasks = pending.tasks
let forwarded = forward(alias[alias_slots[row]])
let running_task = forwarded
let env = Envelope {
    bundle: Bundle {
        tasks: [running_task, worker(75)],
    },
    tail: tail_tasks[1],
}
let bundle_root = env.bundle.tasks
let bundle_alias_root = bundle_root
let bundle_tasks = bundle_alias_root
let bundled = bundle_tasks[0]
let bundle_ready = forward(bundled)
let running = spawn bundle_ready
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
    assert!(rendered.matches("_forward(").count() >= 3);
    assert!(
        rendered
            .matches("getelementptr inbounds [2 x i64], ptr")
            .count()
            >= 3
    );
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
fn emits_async_main_entry_lifecycle_with_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_task_handle_tail_inline_forward_spawn_in_program_mode()
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
    pending.tasks[slots[row]] = worker(first + 31)
}
let tail_tasks = pending.tasks
let forwarded = forward(alias[alias_slots[row]])
let running_task = forwarded
let env = Envelope {
    bundle: Bundle {
        tasks: [running_task, worker(49)],
    },
    tail: tail_tasks[1],
}
let running = spawn forward(env.tail)
let second = await env.bundle.tasks[0]
let extra = await env.bundle.tasks[1]
let tail = await running
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
    assert!(rendered.matches("_forward(").count() >= 3);
    assert!(
        rendered
            .matches("getelementptr inbounds [2 x i64], ptr")
            .count()
            >= 3
    );
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
