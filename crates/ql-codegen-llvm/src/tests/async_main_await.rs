use super::*;

#[test]
fn emits_async_main_entry_lifecycle_with_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_task_handle_tail_inline_forward_await_in_program_mode()
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
        tasks: [running_task, worker(51)],
    },
    tail: tail_tasks[1],
}
let tail = await forward(env.tail)
let second = await env.bundle.tasks[0]
let extra = await env.bundle.tasks[1]
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
fn emits_async_main_entry_lifecycle_with_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_task_handle_bundle_slot_inline_forward_await_in_program_mode()
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
        tasks: [running_task, worker(53)],
    },
    tail: tail_tasks[1],
}
let bundle_slot = Slot { value: INDEX }
let bundle_slot_alias = bundle_slot
let second = await forward(env.bundle.tasks[bundle_slot_alias.value])
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
fn emits_async_main_entry_lifecycle_with_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_task_handle_bundle_inline_forward_await_in_program_mode()
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
        tasks: [running_task, worker(55)],
    },
    tail: tail_tasks[1],
}
let second = await forward(env.bundle.tasks[0])
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
fn emits_async_main_entry_lifecycle_with_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_task_handle_bundle_forward_await_in_program_mode()
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
    pending.tasks[slots[row]] = worker(first + 36)
}
let tail_tasks = pending.tasks
let forwarded = forward(alias[alias_slots[row]])
let running_task = forwarded
let env = Envelope {
    bundle: Bundle {
        tasks: [running_task, worker(68)],
    },
    tail: tail_tasks[1],
}
let bundle_tasks = env.bundle.tasks
let bundled = bundle_tasks[0]
let bundle_ready = forward(bundled)
let second = await bundle_ready
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
fn emits_async_main_entry_lifecycle_with_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_task_handle_bundle_alias_forward_await_in_program_mode()
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
    pending.tasks[slots[row]] = worker(first + 37)
}
let tail_tasks = pending.tasks
let forwarded = forward(alias[alias_slots[row]])
let running_task = forwarded
let env = Envelope {
    bundle: Bundle {
        tasks: [running_task, worker(69)],
    },
    tail: tail_tasks[1],
}
let bundle_root = env.bundle.tasks
let bundle_tasks = bundle_root
let bundled = bundle_tasks[0]
let bundle_ready = forward(bundled)
let second = await bundle_ready
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
fn emits_async_main_entry_lifecycle_with_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_task_handle_bundle_chain_forward_await_in_program_mode()
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
    pending.tasks[slots[row]] = worker(first + 38)
}
let tail_tasks = pending.tasks
let forwarded = forward(alias[alias_slots[row]])
let running_task = forwarded
let env = Envelope {
    bundle: Bundle {
        tasks: [running_task, worker(70)],
    },
    tail: tail_tasks[1],
}
let bundle_root = env.bundle.tasks
let bundle_alias_root = bundle_root
let bundle_tasks = bundle_alias_root
let bundled = bundle_tasks[0]
let bundle_ready = forward(bundled)
let second = await bundle_ready
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
fn emits_async_main_entry_lifecycle_with_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_task_handle_bundle_alias_inline_forward_await_in_program_mode()
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
    pending.tasks[slots[row]] = worker(first + 39)
}
let tail_tasks = pending.tasks
let forwarded = forward(alias[alias_slots[row]])
let running_task = forwarded
let env = Envelope {
    bundle: Bundle {
        tasks: [running_task, worker(71)],
    },
    tail: tail_tasks[1],
}
let bundle_root = env.bundle.tasks
let bundle_tasks = bundle_root
let second = await forward(bundle_tasks[0])
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
fn emits_async_main_entry_lifecycle_with_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_task_handle_bundle_chain_inline_forward_await_in_program_mode()
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
    pending.tasks[slots[row]] = worker(first + 40)
}
let tail_tasks = pending.tasks
let forwarded = forward(alias[alias_slots[row]])
let running_task = forwarded
let env = Envelope {
    bundle: Bundle {
        tasks: [running_task, worker(72)],
    },
    tail: tail_tasks[1],
}
let bundle_root = env.bundle.tasks
let bundle_alias_root = bundle_root
let bundle_tasks = bundle_alias_root
let second = await forward(bundle_tasks[0])
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
fn emits_async_main_entry_lifecycle_with_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_task_handle_queued_local_inline_forward_await_in_program_mode()
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
        tasks: [running_task, worker(57)],
    },
    tail: tail_tasks[1],
}
let queue_root = env.bundle.tasks
let queue_alias_root = queue_root
let queued_tasks = queue_alias_root
let queued = queued_tasks[0]
let queued_alias = queued
let queued_final = queued_alias
let second = await forward(queued_final)
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
    assert!(rendered.matches("store ptr").count() >= 9);
    assert!(!rendered.contains("does not support field or index projections yet"));
    assert!(!rendered.contains("does not support assignment to field or index projections yet"));
}

#[test]
fn emits_async_main_entry_lifecycle_with_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_task_handle_queued_local_forward_await_in_program_mode()
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
        tasks: [running_task, worker(59)],
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
let second = await queued_ready
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
fn emits_async_main_entry_lifecycle_with_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_task_handle_queued_root_inline_forward_await_in_program_mode()
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
        tasks: [running_task, worker(61)],
    },
    tail: tail_tasks[1],
}
let queued_tasks = env.bundle.tasks
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
fn emits_async_main_entry_lifecycle_with_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_task_handle_queued_root_forward_await_in_program_mode()
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
        tasks: [running_task, worker(63)],
    },
    tail: tail_tasks[1],
}
let queued_tasks = env.bundle.tasks
let queued = queued_tasks[0]
let queued_ready = forward(queued)
let second = await queued_ready
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
fn emits_async_main_entry_lifecycle_with_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_task_handle_queued_root_alias_forward_await_in_program_mode()
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
    pending.tasks[slots[row]] = worker(first + 32)
}
let tail_tasks = pending.tasks
let forwarded = forward(alias[alias_slots[row]])
let running_task = forwarded
let env = Envelope {
    bundle: Bundle {
        tasks: [running_task, worker(64)],
    },
    tail: tail_tasks[1],
}
let queue_root = env.bundle.tasks
let queued_tasks = queue_root
let queued = queued_tasks[0]
let queued_ready = forward(queued)
let second = await queued_ready
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
fn emits_async_main_entry_lifecycle_with_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_task_handle_queued_root_chain_forward_await_in_program_mode()
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
    pending.tasks[slots[row]] = worker(first + 33)
}
let tail_tasks = pending.tasks
let forwarded = forward(alias[alias_slots[row]])
let running_task = forwarded
let env = Envelope {
    bundle: Bundle {
        tasks: [running_task, worker(65)],
    },
    tail: tail_tasks[1],
}
let queue_root = env.bundle.tasks
let queue_alias_root = queue_root
let queued_tasks = queue_alias_root
let queued = queued_tasks[0]
let queued_ready = forward(queued)
let second = await queued_ready
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
fn emits_async_main_entry_lifecycle_with_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_task_handle_queued_root_alias_inline_forward_await_in_program_mode()
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
    pending.tasks[slots[row]] = worker(first + 34)
}
let tail_tasks = pending.tasks
let forwarded = forward(alias[alias_slots[row]])
let running_task = forwarded
let env = Envelope {
    bundle: Bundle {
        tasks: [running_task, worker(66)],
    },
    tail: tail_tasks[1],
}
let queue_root = env.bundle.tasks
let queued_tasks = queue_root
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
