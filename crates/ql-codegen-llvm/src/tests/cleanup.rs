use super::*;

#[test]
fn emits_simple_defer_call_cleanup_lowering() {
    let rendered = emit(
        r#"
extern "c" fn first()

fn main() -> Int {
defer first()
return 0
}
"#,
    );

    assert!(rendered.contains("define i64 @ql_1_main()"));
    assert!(rendered.contains("define i32 @main()"));
    assert!(rendered.contains("call void @first()"));
    assert!(!rendered.contains("does not support cleanup lowering yet"));
}

#[test]
fn dedupes_cleanup_and_for_lowering_diagnostics() {
    let messages = emit_error(
        r#"
extern "c" fn first()

fn main() -> Int {
defer first()
for value in 0 {
    break
}
return 0
}
"#,
    );

    assert_eq!(
        messages
            .iter()
            .filter(|message| {
                message.as_str()
                    == "LLVM IR backend foundation does not support cleanup lowering yet"
            })
            .count(),
        0
    );
    assert_eq!(
        messages
            .iter()
            .filter(|message| {
                message.as_str() == "LLVM IR backend foundation does not support `for` lowering yet"
            })
            .count(),
        1
    );
    assert!(messages.iter().all(|message| {
        !message.contains("could not resolve LLVM type for local")
            && !message.contains("could not infer LLVM type for MIR local")
    }));
}

#[test]
fn emits_cleanup_match_lowering() {
    let rendered = emit(
        r#"
extern "c" fn first()
extern "c" fn second()

fn enabled() -> Bool {
return true
}

fn main() -> Int {
let flag = true
defer match flag {
    true if enabled() => first(),
    _ => second(),
}
return 0
}
"#,
    );

    assert!(rendered.contains("call void @first()"));
    assert!(rendered.contains("call void @second()"));
    assert!(!rendered.contains("does not support cleanup lowering yet"));
    assert!(!rendered.contains("does not support `match` lowering yet"));
}

#[test]
fn emits_cleanup_match_callable_guard_alias_lowering() {
    let rendered = emit(
        r#"
use READY as ready
use AMOUNT as amount

extern "c" fn sink(value: Int)
extern "c" fn second()

fn enabled() -> Bool {
return true
}

fn measure() -> Int {
return 7
}

const READY: () -> Bool = enabled
const AMOUNT: () -> Int = measure

fn main() -> Int {
let flag = true
defer match flag {
    true if ready() => sink(amount()),
    _ => second(),
}
return 0
}
"#,
    );

    assert!(rendered.contains("call i1 %t"));
    assert!(rendered.contains("call i64 %t"));
    assert!(rendered.contains("call void @sink(i64"));
    assert!(!rendered.contains("does not support cleanup lowering yet"));
}

#[test]
fn emits_cleanup_match_binding_arm_lowering() {
    let rendered = emit(
        r#"
extern "c" fn sink(value: Int)
extern "c" fn fallback()

fn main() -> Int {
let value = 42
defer match value {
    current if current == 42 => sink(current),
    _ => fallback(),
}
return 0
}
"#,
    );

    assert!(rendered.contains("icmp eq i64"));
    assert!(rendered.contains("call void @sink(i64"));
    assert!(rendered.contains("call void @fallback()"));
    assert!(!rendered.contains("does not support cleanup lowering yet"));
    assert!(!rendered.contains("does not support `match` lowering yet"));
}

#[test]
fn emits_cleanup_if_and_match_block_lowering_with_async_bodies() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::TaskAwait,
        RuntimeCapability::AsyncIteration,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
extern "c" fn step(value: Int)

async fn worker(value: Int) -> Int {
return value
}

async fn main() -> Int {
let branch = true
defer if branch {
    let values = [worker(1), worker(2)]
    for await value in values {
        step(value);
    }
} else {
    step(0);
}
defer match branch {
    true => {
        let values = [worker(3), worker(4)]
        for await value in values {
            step(value);
        }
    }
    false => {
        step(0);
    }
}
return 0
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.contains("cleanup_then"));
    assert!(rendered.contains("cleanup_else"));
    assert!(rendered.contains("cleanup_for_cond"));
    assert!(rendered.matches("call ptr @qlrt_task_await").count() >= 3);
    assert!(rendered.matches("call void @step(i64").count() >= 3);
    assert!(!rendered.contains("does not support cleanup lowering yet"));
    assert!(!rendered.contains("does not support `for await` lowering yet"));
}

#[test]
fn emits_callable_value_control_flow_in_async_and_cleanup_paths() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
use add_one as item_alias
use APPLY as const_alias
use APPLY_CLOSURE_CONST as closure_const
use APPLY_CLOSURE_STATIC as closure_static
use worker as async_item_alias
use ASYNC_APPLY as async_const_alias

extern "c" fn sink(value: Int)

fn add_one(value: Int) -> Int {
return value + 1
}

async fn worker(value: Int) -> Int {
return value + 10
}

const APPLY: (Int) -> Int = add_one
const APPLY_CLOSURE_CONST: (Int) -> Int = (value: Int) => value + 2
static APPLY_CLOSURE_STATIC: (Int) -> Int = (value: Int) => value + 3
const ASYNC_APPLY: (Int) -> Task[Int] = worker

async fn main() -> Int {
let branch = true
let picked_sync = if branch { item_alias } else { const_alias }
let picked_closure = match branch {
    true => closure_const,
    false => closure_static,
}
let picked_async = if branch { async_item_alias } else { async_const_alias }
let matched_async = match branch {
    true => async_const_alias,
    false => async_item_alias,
}
var total = picked_sync(10) + picked_closure(20)
total = total + await picked_async(30);
total = total + await matched_async(40);
defer {
    let cleanup_sync = if branch { closure_const } else { item_alias }
    sink(cleanup_sync(50));
    let cleanup_async = match branch {
        true => async_item_alias,
        false => async_const_alias,
    }
    sink(await cleanup_async(60));
}
return total
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.contains("br i1"));
    assert!(rendered.matches("call i64 %t").count() >= 2);
    assert!(rendered.matches("call ptr %t").count() >= 2);
    assert!(rendered.matches("call ptr @qlrt_task_await").count() >= 3);
    assert!(rendered.matches("call void @sink(i64").count() >= 2);
    assert!(!rendered.contains("does not support imported value lowering yet"));
    assert!(!rendered.contains("does not support cleanup lowering yet"));
}

#[test]
fn emits_cleanup_block_sequence_lowering() {
    let rendered = emit(
        r#"
extern "c" fn first()
extern "c" fn second()

fn main() -> Int {
defer {
    first();
    second()
}
return 0
}
"#,
    );

    assert!(rendered.contains("call void @first()"));
    assert!(rendered.contains("call void @second()"));
    assert!(!rendered.contains("does not support cleanup lowering yet"));
}

#[test]
fn emits_cleanup_block_let_bindings() {
    let rendered = emit(
        r#"
extern "c" fn sink(value: Int)

fn amount() -> Int {
return 41
}

fn main() -> Int {
defer {
    let value = amount()
    sink(value)
}
return 0
}
"#,
    );

    assert!(rendered.contains("_amount()"));
    assert!(rendered.contains("call i64 @ql_"));
    assert!(rendered.contains("call void @sink(i64"));
    assert!(!rendered.contains("does not support cleanup lowering yet"));
}

#[test]
fn emits_cleanup_block_let_destructuring() {
    let rendered = emit(
        r#"
struct Pair {
left: Int,
right: Int,
}

extern "c" fn sink(value: Int)

fn values() -> (Int, Int) {
return (4, 6)
}

fn pair_value() -> Pair {
return Pair { left: 20, right: 22 }
}

fn main() -> Int {
defer {
    let (first, _) = values();
    let Pair { left, right: current } = pair_value();
    sink(first);
    sink(left);
    sink(current)
}
return 0
}
"#,
    );

    assert!(rendered.matches("extractvalue { i64, i64 }").count() >= 2);
    assert_eq!(rendered.matches("call void @sink(i64").count(), 3);
    assert!(!rendered.contains("does not support cleanup lowering yet"));
}

#[test]
fn emits_cleanup_block_assignment_expr_lowering() {
    let rendered = emit(
        r#"
struct State {
current: Int,
pair: (Int, Int),
values: [Int; 3],
}

fn main() -> Int {
var total = 1
var index = 1
var state = State {
    current: 2,
    pair: (3, 4),
    values: [5, 6, 7],
}
defer {
    total = 8;
    state.current = total + 1;
    state.pair[0] = state.current + 1;
    state.values[index] = state.pair[0] + 1;
}
return total + state.current + state.pair[0] + state.values[1]
}
"#,
    );

    assert!(rendered.matches("store i64").count() >= 4);
    assert!(rendered.contains("getelementptr inbounds [3 x i64]"));
    assert!(!rendered.contains("does not support cleanup lowering yet"));
    assert!(!rendered.contains("does not support assignment expressions yet"));
}

#[test]
fn emits_cleanup_value_assignment_expr_lowering() {
    let rendered = emit(
        r#"
struct State {
current: Int,
values: [Int; 3],
}

fn forward(value: Int) -> Int {
return value
}

fn main() -> Int {
var index = 1
var state = State {
    current: 2,
    values: [3, 4, 5],
}
defer {
    forward(state.current = 6);
    forward({
        state.values[index] = state.current + 1
    });
}
return state.current + state.values[1]
}
"#,
    );

    assert!(rendered.matches("store i64").count() >= 2);
    assert!(rendered.matches("call i64 @ql_").count() >= 2);
    assert!(rendered.contains("getelementptr inbounds [3 x i64]"));
    assert!(!rendered.contains("does not support cleanup lowering yet"));
    assert!(!rendered.contains("does not support assignment expressions yet"));
}

#[test]
fn emits_cleanup_if_value_lowering() {
    let rendered = emit(
        r#"
fn forward(value: Int) -> Int {
return value
}

fn main() -> Int {
var value = 0
defer {
    forward(if value == 0 { 1 } else { 2 });
}
return 0
}
"#,
    );

    assert!(rendered.contains("cleanup_then"));
    assert!(rendered.contains("cleanup_else"));
    assert!(rendered.contains("alloca i64"));
    assert!(rendered.contains("call i64 @ql_"));
    assert!(!rendered.contains("does not support cleanup lowering yet"));
}

#[test]
fn emits_cleanup_match_value_lowering() {
    let rendered = emit(
        r#"
fn forward(value: Int) -> Int {
return value
}

fn main() -> Int {
var value = 0
defer {
    forward(match value {
        0 => 1,
        _ => 2,
    });
}
return 0
}
"#,
    );

    assert!(rendered.contains("cleanup_match_arm"));
    assert!(rendered.contains("cleanup_match_end"));
    assert!(rendered.contains("alloca i64"));
    assert!(rendered.contains("call i64 @ql_"));
    assert!(!rendered.contains("does not support cleanup lowering yet"));
}

#[test]
fn emits_cleanup_await_value_lowering() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
async fn worker(value: Int) -> Int {
return value + 1
}

fn forward(value: Int) -> Int {
return value
}

async fn main() -> Int {
defer forward(await worker(1))
return 0
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.contains("call ptr @qlrt_task_await"));
    assert!(rendered.contains("call void @qlrt_task_result_release"));
    assert!(rendered.contains("call i64 @ql_"));
    assert!(!rendered.contains("does not support cleanup lowering yet"));
}

#[test]
fn emits_cleanup_spawn_value_lowering() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
async fn worker(value: Int) -> Int {
return value + 1
}

fn keep(task: Task[Int]) -> Int {
return 0
}

async fn main() -> Int {
defer keep(spawn worker(1))
return 0
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.matches("call ptr @qlrt_executor_spawn").count() >= 2);
    assert!(rendered.contains("call i64 @ql_"));
    assert!(!rendered.contains("does not support cleanup lowering yet"));
}

#[test]
fn emits_cleanup_await_guard_lowering() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
extern "c" fn sink(value: Int)

async fn ready() -> Bool {
return true
}

async fn check(value: Int) -> Bool {
return value == 1
}

async fn main() -> Int {
defer if await ready() {
    sink(1);
}
defer match true {
    true if await check(1) => sink(2),
    _ => sink(3),
}
return 0
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.matches("call ptr @qlrt_task_await").count() >= 2);
    assert!(rendered.contains("cleanup_match_guard"));
    assert!(rendered.contains("cleanup_then"));
    assert!(!rendered.contains("does not support cleanup lowering yet"));
}

#[test]
fn emits_cleanup_awaited_control_flow_roots() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
use ready as async_alias
use APPLY as async_const_alias

extern "c" fn sink(value: Int)

async fn ready(value: Int) -> Bool {
return value == 1
}

const APPLY: (Int) -> Task[Bool] = ready

async fn main() -> Int {
let branch = true
defer if await (if branch { async_alias } else { async_const_alias })(1) {
    sink(1);
}
defer match await (match branch { true => async_const_alias, false => async_alias })(1) {
    true => sink(2),
    false => sink(3),
}
return 0
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.matches("call ptr @qlrt_task_await").count() >= 2);
    assert!(rendered.matches("call ptr %t").count() >= 2);
    assert!(rendered.contains("cleanup_match_end"));
    assert!(rendered.contains("cleanup_then"));
    assert!(!rendered.contains("does not support cleanup lowering yet"));
}

#[test]
fn emits_cleanup_awaited_projection_async_callable_control_flow_scrutinees() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
use load_state as async_alias
use LOAD as async_const_alias

struct State {
value: Int,
}

extern "c" fn sink(value: Int)

async fn load_state(value: Int) -> State {
return State { value: value }
}

const LOAD: (Int) -> Task[State] = load_state

async fn main() -> Int {
let branch = true
defer match (await (if branch { async_alias } else { async_const_alias })(15)).value {
    15 => sink(1),
    _ => sink(0),
}
defer match (await (match branch { true => async_const_alias, false => async_alias })(16)).value {
    16 => sink(2),
    _ => sink(3),
}
return 0
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.matches("call ptr @qlrt_task_await").count() >= 2);
    assert!(rendered.matches("call ptr %t").count() >= 2);
    assert!(rendered.contains("cleanup_match_end"));
    assert!(!rendered.contains("does not support cleanup lowering yet"));
}

#[test]
fn emits_cleanup_awaited_helper_and_inline_guards() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
use load_state as async_alias
use LOAD as async_const_alias
use matches as helper_alias

struct State {
value: Int,
}

extern "c" fn sink(value: Int)

async fn load_state(value: Int) -> State {
return State {
    value: value,
}
}

fn matches(expected: Int, state: State) -> Bool {
return state.value == expected
}

const LOAD: (Int) -> Task[State] = load_state

async fn main() -> Int {
let branch = true
defer if helper_alias(13, await (if branch { async_alias } else { async_const_alias })(13)) {
    sink(1);
}
defer if State { value: (await (match branch { true => async_const_alias, false => async_alias })(14)).value }.value == 14 {
    sink(2);
}
return 0
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.matches("call ptr @qlrt_task_await").count() >= 2);
    assert!(rendered.matches("call ptr %t").count() >= 2);
    assert!(rendered.matches("call i1 @ql_").count() >= 1);
    assert!(rendered.contains("cleanup_then"));
    assert!(!rendered.contains("does not support cleanup lowering yet"));
}

#[test]
fn emits_cleanup_awaited_nested_runtime_projection_guards() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
use load_state as async_alias
use LOAD as async_const_alias

struct Slot {
value: Int,
}

struct State {
slot: Slot,
}

extern "c" fn sink(value: Int)

async fn load_state(value: Int) -> State {
return State {
    slot: Slot { value: value },
}
}

fn wrap(state: State) -> State {
return state
}

fn offset(value: Int) -> Int {
return value - 11
}

fn matches(value: Int, expected: Int) -> Bool {
return value == expected
}

const LOAD: (Int) -> Task[State] = load_state

async fn main() -> Int {
let branch = true
defer if wrap(await (if branch { async_alias } else { async_const_alias })(13)).slot.value == 13 {
    sink(1);
}
defer match true {
    true if matches(value: [wrap(await (match branch { true => async_const_alias, false => async_alias })(15)).slot.value, 0][offset(11)], expected: 15) => sink(2),
    _ => sink(3),
}
return 0
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.matches("call ptr @qlrt_task_await").count() >= 2);
    assert!(rendered.matches("call ptr %t").count() >= 2);
    assert!(rendered.matches("call i1 @ql_").count() >= 1);
    assert!(rendered.contains("cleanup_match_guard"));
    assert!(rendered.contains("cleanup_then"));
    assert!(!rendered.contains("does not support cleanup lowering yet"));
}

#[test]
fn emits_cleanup_awaited_helper_and_inline_scrutinees() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
use load_state as async_alias
use LOAD as async_const_alias
use matches as helper_alias

struct State {
value: Int,
}

extern "c" fn sink(value: Int)

async fn load_state(value: Int) -> State {
return State {
    value: value,
}
}

fn matches(expected: Int, state: State) -> Bool {
return state.value == expected
}

const LOAD: (Int) -> Task[State] = load_state

async fn main() -> Int {
let branch = true
defer match helper_alias(13, await (if branch { async_alias } else { async_const_alias })(13)) {
    true => sink(1),
    false => sink(0),
}
defer match State { value: (await (match branch { true => async_const_alias, false => async_alias })(14)).value }.value {
    14 => sink(2),
    _ => sink(3),
}
return 0
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.matches("call ptr @qlrt_task_await").count() >= 2);
    assert!(rendered.matches("call ptr %t").count() >= 2);
    assert!(rendered.matches("call i1 @ql_").count() >= 1);
    assert!(rendered.contains("cleanup_match_end"));
    assert!(!rendered.contains("does not support cleanup lowering yet"));
}

#[test]
fn emits_cleanup_awaited_nested_runtime_projection_scrutinees() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
use load_state as async_alias
use LOAD as async_const_alias

struct Slot {
value: Int,
}

struct State {
slot: Slot,
}

extern "c" fn sink(value: Int)

async fn load_state(value: Int) -> State {
return State { slot: Slot { value: value } }
}

fn wrap(state: State) -> State {
return state
}

fn offset(value: Int) -> Int {
return value - 11
}

const LOAD: (Int) -> Task[State] = load_state

async fn main() -> Int {
let branch = true
defer match wrap(await (if branch { async_alias } else { async_const_alias })(13)).slot.value {
    13 => sink(1),
    _ => sink(0),
}
defer match [wrap(await (match branch { true => async_const_alias, false => async_alias })(15)).slot.value, 0][offset(11)] {
    15 => sink(2),
    _ => sink(3),
}
return 0
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.matches("call ptr @qlrt_task_await").count() >= 2);
    assert!(rendered.matches("call ptr %t").count() >= 2);
    assert!(rendered.contains("cleanup_match_end"));
    assert!(!rendered.contains("does not support cleanup lowering yet"));
}

#[test]
fn emits_cleanup_awaited_aggregate_binding_scrutinees() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
use load_state as state_alias
use load_pair as pair_alias
use load_values as values_alias
use LOAD_STATE as state_const_alias
use LOAD_PAIR as pair_const_alias
use LOAD_VALUES as values_const_alias

struct Slot {
ready: Bool,
value: Int,
}

struct State {
slot: Slot,
}

extern "c" fn sink(value: Int)

async fn load_state(value: Int) -> State {
return State { slot: Slot { ready: true, value: value } }
}

async fn load_pair(value: Int) -> (Int, Int) {
return (value, value + 1)
}

async fn load_values(value: Int) -> [Int; 3] {
return [value, value + 1, value + 2]
}

const LOAD_STATE: (Int) -> Task[State] = load_state
const LOAD_PAIR: (Int) -> Task[(Int, Int)] = load_pair
const LOAD_VALUES: (Int) -> Task[[Int; 3]] = load_values

async fn main() -> Int {
let branch = true
defer match await (if branch { state_alias } else { state_const_alias })(13) {
    current => sink(current.slot.value),
}
defer match await (match branch { true => pair_const_alias, false => pair_alias })(20) {
    current => sink(current[0] + current[1]),
}
defer match await (if branch { values_alias } else { values_const_alias })(30) {
    current => sink(current[0] + current[2]),
}
return 0
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.matches("call ptr @qlrt_task_await").count() >= 3);
    assert!(rendered.matches("call ptr %t").count() >= 3);
    assert!(rendered.contains("getelementptr inbounds { { i1, i64 } }"));
    assert!(rendered.contains("getelementptr inbounds { i64, i64 }"));
    assert!(rendered.contains("getelementptr inbounds [3 x i64]"));
    assert!(rendered.contains("cleanup_match_end"));
    assert!(!rendered.contains("does not support cleanup lowering yet"));
}

#[test]
fn emits_cleanup_awaited_aggregate_destructuring_scrutinees() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
use load_state as state_alias
use load_pair as pair_alias
use LOAD_STATE as state_const_alias
use LOAD_PAIR as pair_const_alias

struct Slot {
value: Int,
}

struct State {
slot: Slot,
}

extern "c" fn sink(value: Int)

async fn load_state(value: Int) -> State {
return State { slot: Slot { value: value } }
}

async fn load_pair(value: Int) -> (Int, Int) {
return (value, value + 1)
}

const LOAD_STATE: (Int) -> Task[State] = load_state
const LOAD_PAIR: (Int) -> Task[(Int, Int)] = load_pair

async fn main() -> Int {
let branch = true
defer {
    sink(match await (if branch { pair_alias } else { pair_const_alias })(20) {
        (left, right) => left + right,
    });
}
defer match await (match branch { true => state_const_alias, false => state_alias })(13) {
    State { slot: Slot { value } } if value == 13 => sink(value),
    _ => sink(0),
}
return 0
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.matches("call ptr @qlrt_task_await").count() >= 2);
    assert!(rendered.matches("call ptr %t").count() >= 2);
    assert!(rendered.contains("extractvalue { i64, i64 }"));
    assert!(rendered.contains("extractvalue { { i64 } }"));
    assert!(rendered.contains("icmp eq i64"));
    assert!(rendered.contains("cleanup_match_end"));
    assert!(!rendered.contains("does not support cleanup lowering yet"));
}

#[test]
fn emits_assignment_expr_value_lowering() {
    let rendered = emit(
        r#"
struct State {
current: Int,
values: [Int; 3],
}

fn forward(value: Int) -> Int {
return value
}

fn main() -> Int {
var index = 1
var state = State {
    current: 2,
    values: [3, 4, 5],
}
let first = forward(state.current = 6)
let second = {
    state.values[index] = state.current + 1
}
return first + second + state.values[1]
}
"#,
    );

    assert!(rendered.matches("store i64").count() >= 2);
    assert!(rendered.matches("call i64 @ql_").count() >= 1);
    assert!(rendered.contains("getelementptr inbounds [3 x i64]"));
    assert!(!rendered.contains("does not support assignment expressions yet"));
}

#[test]
fn emits_guard_assignment_expr_lowering() {
    let rendered = emit(
        r#"
fn forward(value: Int) -> Int {
return value
}

fn main() -> Int {
var cleanup_enabled = false
defer if cleanup_enabled = true { forward(1) } else { forward(0) }
return 0
}
"#,
    );

    assert!(rendered.matches("store i1").count() >= 1);
    assert!(rendered.contains("cleanup_then"));
    assert!(rendered.contains("cleanup_else"));
    assert!(!rendered.contains("does not support cleanup lowering yet"));
    assert!(!rendered.contains("does not support assignment expressions yet"));
}

#[test]
fn emits_guard_assignment_call_arg_lowering() {
    let rendered = emit(
        r#"
struct State {
value: Int,
}

fn allow(state: State) -> Bool {
return state.value == 1
}

fn main() -> Int {
var state = State { value: 0 }
return match 1 {
    1 if allow(state = State { value: 1 }) => state.value,
    _ => 0,
}
}
"#,
    );

    assert!(rendered.contains("bb0_match_guard0:"));
    assert!(rendered.contains("call i1 @ql_"));
    assert!(rendered.matches("insertvalue { i64 }").count() >= 1);
    assert!(rendered.matches("store { i64 }").count() >= 1);
    assert!(!rendered.contains("does not support `match` lowering yet"));
    assert!(!rendered.contains("does not support assignment expressions yet"));
}

#[test]
fn emits_guard_if_value_call_arg_lowering() {
    let rendered = emit(
        r#"
struct State {
value: Int,
}

fn allow(state: State) -> Bool {
return state.value == 1
}

fn main() -> Int {
let ready = true
return match 1 {
    1 if allow(if ready { State { value: 1 } } else { State { value: 2 } }) => 10,
    _ => 0,
}
}
"#,
    );

    assert!(rendered.contains("bb0_match_guard0:"));
    assert!(rendered.contains("guard_if_then"));
    assert!(rendered.contains("guard_if_else"));
    assert!(rendered.contains("call i1 @ql_"));
    assert!(!rendered.contains("does not support `match` lowering yet"));
}

#[test]
fn emits_guard_match_value_call_arg_lowering() {
    let rendered = emit(
        r#"
struct State {
value: Int,
}

fn allow(state: State) -> Bool {
return state.value == 1
}

fn main() -> Int {
let ready = true
return match 1 {
    1 if allow(match ready { true => State { value: 1 }, false => State { value: 2 } }) => 10,
    _ => 0,
}
}
"#,
    );

    assert!(rendered.contains("bb0_match_guard0:"));
    assert!(rendered.contains("guard_match_arm"));
    assert!(rendered.contains("guard_match_end"));
    assert!(rendered.contains("call i1 @ql_"));
    assert!(!rendered.contains("does not support `match` lowering yet"));
}

#[test]
fn emits_guard_if_callable_callee_lowering() {
    let rendered = emit(
        r#"
struct State {
value: Int,
}

fn allow_one(state: State) -> Bool {
return state.value == 1
}

fn allow_two(state: State) -> Bool {
return state.value == 2
}

fn main() -> Int {
let ready = true
return match 1 {
    1 if (if ready { allow_one } else { allow_two })(State { value: 1 }) => 10,
    _ => 0,
}
}
"#,
    );

    assert!(rendered.contains("bb0_match_guard0:"));
    assert!(rendered.contains("guard_call_if_then"));
    assert!(rendered.contains("guard_call_if_else"));
    assert!(rendered.matches("call i1 @ql_").count() >= 2);
    assert!(!rendered.contains("does not support `match` lowering yet"));
}

#[test]
fn emits_callable_control_flow_callee_roots_in_guards_and_cleanup() {
    let rendered = emit(
        r#"
use APPLY as const_alias
use APPLY_CLOSURE_CONST as closure_const
use APPLY_CLOSURE_STATIC as closure_static

fn add_one(value: Int) -> Int {
return value + 1
}

const APPLY: (Int) -> Int = add_one
const APPLY_CLOSURE_CONST: (Int) -> Int = (value: Int) => value + 2
static APPLY_CLOSURE_STATIC: (Int) -> Int = (value: Int) => value + 3

fn main() -> Int {
let branch = true
defer (if branch { const_alias } else { closure_const })(40)
defer (match branch {
    true => closure_static,
    false => const_alias,
})(1)
return match 1 {
    1 if (if branch { closure_const } else { const_alias })(1) == 3 => 10,
    1 if (match branch { true => closure_static, false => const_alias })(2) == 5 => 20,
    _ => 0,
}
}
"#,
    );

    assert!(rendered.contains("guard_call_if_then"));
    assert!(rendered.contains("guard_call_match_arm"));
    assert!(rendered.matches("call i64 %t").count() >= 4);
    assert!(!rendered.contains("does not support imported value lowering yet"));
    assert!(!rendered.contains("does not support cleanup lowering yet"));
    assert!(!rendered.contains("does not support callable const/static values yet"));
}

#[test]
fn emits_cleanup_block_while_lowering() {
    let rendered = emit(
        r#"
fn running() -> Bool {
return false
}

fn step() {
return
}

fn main() -> Int {
defer {
    while running() {
        step()
    }
}
return 0
}
"#,
    );

    assert!(rendered.contains("cleanup_while_cond"));
    assert!(rendered.contains("cleanup_while_body"));
    assert!(rendered.contains("call i1 @ql_0_running()"));
    assert!(rendered.contains("call void @ql_1_step()"));
    assert!(!rendered.contains("does not support cleanup lowering yet"));
}

#[test]
fn emits_cleanup_block_while_break_continue_lowering() {
    let rendered = emit(
        r#"
extern "c" fn running() -> Bool
extern "c" fn stop() -> Bool
extern "c" fn step()
extern "c" fn after()

fn main() -> Int {
defer {
    while running() {
        if stop() {
            break
        };
        step();
        continue;
        after();
    }
}
return 0
}
"#,
    );

    assert!(rendered.contains("cleanup_while_cond"));
    assert!(rendered.contains("cleanup_while_body"));
    assert!(rendered.contains("call i1 @running()"));
    assert!(rendered.contains("call i1 @stop()"));
    assert!(rendered.contains("call void @step()"));
    assert!(!rendered.contains("call void @after()"));
    assert!(!rendered.contains("does not support cleanup lowering yet"));
}

#[test]
fn emits_cleanup_block_loop_break_continue_lowering() {
    let rendered = emit(
        r#"
extern "c" fn stop() -> Bool
extern "c" fn step()
extern "c" fn after()

fn main() -> Int {
defer {
    loop {
        if stop() {
            break
        };
        step();
        continue;
        after();
    }
}
return 0
}
"#,
    );

    assert!(rendered.contains("cleanup_loop_body"));
    assert!(rendered.contains("cleanup_loop_end"));
    assert!(rendered.contains("call i1 @stop()"));
    assert!(rendered.contains("call void @step()"));
    assert!(!rendered.contains("call void @after()"));
    assert!(!rendered.contains("does not support cleanup lowering yet"));
}

#[test]
fn emits_cleanup_block_for_lowering_for_fixed_shapes() {
    let rendered = emit(
        r#"
extern "c" fn stop() -> Bool
extern "c" fn step(value: Int)
extern "c" fn finish(value: Int)

fn main() -> Int {
defer {
    for value in [1, 2] {
        if stop() {
            break
        };
        step(value);
        continue;
        finish(value);
    }
    for item in (3, 4) {
        step(item);
        break;
        finish(item);
    }
}
return 0
}
"#,
    );

    assert!(rendered.contains("cleanup_for_cond"));
    assert!(rendered.contains("cleanup_for_tuple_item"));
    assert!(rendered.contains("call i1 @stop()"));
    assert!(rendered.contains("call void @step(i64"));
    assert!(!rendered.contains("call void @finish(i64"));
    assert!(!rendered.contains("does not support cleanup lowering yet"));
    assert!(!rendered.contains("does not support `for` lowering yet"));
}

#[test]
fn emits_cleanup_block_for_await_lowering_for_fixed_shapes() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::TaskAwait,
        RuntimeCapability::AsyncIteration,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
extern "c" fn step(value: Int)
extern "c" fn finish(value: Int)

async fn worker(value: Int) -> Int {
return value
}

async fn main() -> Int {
let tasks = (worker(3), worker(4))
defer {
    for await value in [1, 2] {
        step(value);
        continue;
        finish(value);
    }
    for await item in tasks {
        step(item);
        break;
        finish(item);
    }
}
return 0
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.contains("cleanup_for_cond"));
    assert!(rendered.contains("cleanup_for_tuple_item"));
    assert!(rendered.contains("call void @step(i64"));
    assert!(!rendered.contains("call void @finish(i64"));
    assert!(rendered.matches("call ptr @qlrt_task_await").count() >= 3);
    assert!(
        rendered
            .matches("call void @qlrt_task_result_release")
            .count()
            >= 3
    );
    assert!(!rendered.contains("does not support cleanup lowering yet"));
    assert!(!rendered.contains("does not support `for await` lowering yet"));
}

#[test]
fn emits_cleanup_block_for_await_lowering_for_call_roots() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::TaskAwait,
        RuntimeCapability::AsyncIteration,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
use tasks as load_tasks

struct TaskArrayPayload {
tasks: [Task[Int]; 2],
}

struct TaskEnvelope {
payload: TaskArrayPayload,
}

extern "c" fn step(value: Int)

async fn worker(value: Int) -> Int {
return value
}

fn tasks(base: Int) -> [Task[Int]; 2] {
return [worker(base), worker(base + 1)]
}

fn task_env(base: Int) -> TaskEnvelope {
return TaskEnvelope {
    payload: TaskArrayPayload {
        tasks: [worker(base), worker(base + 1)],
    },
}
}

async fn main() -> Int {
defer {
    for await value in tasks(1) {
        step(value);
    }
    for await item in load_tasks(3) {
        step(item);
    }
    for await tail in task_env(5).payload.tasks {
        step(tail);
    }
}
return 0
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.contains("cleanup_for_cond"));
    assert!(rendered.matches("call void @step(i64").count() >= 1);
    assert!(rendered.matches("call ptr @qlrt_task_await").count() >= 3);
    assert!(
        rendered
            .matches("call void @qlrt_task_result_release")
            .count()
            >= 3
    );
    assert!(!rendered.contains("does not support cleanup lowering yet"));
    assert!(!rendered.contains("does not support `for await` lowering yet"));
}

#[test]
fn emits_cleanup_block_for_await_lowering_for_direct_control_flow_roots() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::TaskAwait,
        RuntimeCapability::AsyncIteration,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
extern "c" fn step(value: Int)

async fn worker(value: Int) -> Int {
return value
}

async fn load_values(base: Int) -> [Int; 2] {
return [base, base + 1]
}

async fn load_tasks(base: Int) -> [Task[Int]; 2] {
return [worker(base), worker(base + 1)]
}

async fn load_task_pair(base: Int) -> (Task[Int], Task[Int]) {
return (worker(base), worker(base + 1))
}

async fn main() -> Int {
let branch = true
var tasks = [worker(0), worker(0)]
defer {
    for await value in ({ let current = [worker(1), worker(2)]; current }) {
        step(value);
    }
    for await value in (tasks = [worker(3), worker(4)]) {
        step(value);
    }
    for await value in (if branch { [worker(5), worker(6)] } else { [worker(7), worker(8)] }) {
        step(value);
    }
    for await item in (match branch {
        true => [worker(9), worker(10)],
        false => [worker(11), worker(12)],
    }) {
        step(item);
    }
    for await scalar in await load_values(13) {
        step(scalar);
    }
    for await awaited in await load_tasks(15) {
        step(awaited);
    }
    for await pair in await load_task_pair(17) {
        step(pair);
    }
}
return 0
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.contains("cleanup_for_cond"));
    assert!(rendered.contains("cleanup_match_arm"));
    assert!(rendered.matches("call void @step(i64").count() >= 1);
    assert!(rendered.matches("call ptr @qlrt_task_await").count() >= 7);
    assert!(
        rendered
            .matches("call void @qlrt_task_result_release")
            .count()
            >= 5
    );
    assert!(!rendered.contains("does not support cleanup lowering yet"));
    assert!(!rendered.contains("does not support `for await` lowering yet"));
}

#[test]
fn emits_cleanup_block_for_await_lowering_for_direct_question_roots() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::TaskAwait,
        RuntimeCapability::AsyncIteration,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
extern "c" fn step(value: Int)

async fn worker(value: Int) -> Int {
return value
}

fn task_array() -> [Task[Int]; 2] {
return [worker(1), worker(2)]
}

fn task_pair() -> (Task[Int], Task[Int]) {
return (worker(3), worker(4))
}

async fn main() -> Int {
defer {
    for await value in task_array()? {
        step(value);
    }
    for await item in task_pair()? {
        step(item);
    }
    for await tail in ({ let tasks = [worker(5), worker(6)]; tasks })? {
        step(tail);
    }
}
return 0
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.contains("cleanup_for_cond"));
    assert!(rendered.matches("call void @step(i64").count() >= 1);
    assert!(rendered.matches("call ptr @qlrt_task_await").count() >= 4);
    assert!(
        rendered
            .matches("call void @qlrt_task_result_release")
            .count()
            >= 4
    );
    assert!(!rendered.contains("does not support cleanup lowering yet"));
    assert!(!rendered.contains("does not support `for await` lowering yet"));
}

#[test]
fn emits_cleanup_block_for_await_lowering_for_scalar_item_roots() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::TaskAwait,
        RuntimeCapability::AsyncIteration,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
use MORE as ITEMS
use BOX as STATE

struct Holder {
values: [Int; 2],
}

extern "c" fn step(value: Int)

const VALUES: [Int; 2] = [1, 2]
static MORE: [Int; 2] = [3, 4]
const BOX: Holder = Holder { values: [5, 6] }

async fn main() -> Int {
defer {
    for await value in VALUES {
        step(value);
    }
    for await item in ITEMS {
        step(item);
    }
    for await projected in STATE.values {
        step(projected);
    }
}
return 0
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.contains("cleanup_for_cond"));
    assert!(rendered.matches("call void @step(i64").count() >= 1);
    assert!(!rendered.contains("does not support cleanup lowering yet"));
    assert!(!rendered.contains("does not support `for await` lowering yet"));
}

#[test]
fn emits_for_await_lowering_for_task_item_roots() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::TaskAwait,
        RuntimeCapability::AsyncIteration,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
use MORE as ITEMS
use TASK_PAIR as PAIRS

extern "c" fn step(value: Int)

const TASKS: [Task[Int]; 2] = [worker(1), worker(2)]
static MORE: [Task[Int]; 2] = [worker(3), worker(4)]
const TASK_PAIR: (Task[Int], Task[Int]) = (worker(5), worker(6))

async fn worker(value: Int) -> Int {
return value
}

async fn main() -> Int {
var total = 0
for await value in TASKS {
    total = total + value
}
for await item in ITEMS {
    total = total + item
}
for await pair in PAIRS {
    total = total + pair
}
defer {
    for await value in TASKS {
        step(value);
    }
    for await item in ITEMS {
        step(item);
    }
    for await pair in PAIRS {
        step(pair);
    }
}
return total
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.contains("cleanup_for_cond"));
    assert!(rendered.contains("cleanup_for_tuple_item"));
    assert!(rendered.matches("call ptr @ql_").count() >= 6);
    assert!(rendered.matches("call ptr @qlrt_task_await").count() >= 6);
    assert!(
        rendered
            .matches("call void @qlrt_task_result_release")
            .count()
            >= 6
    );
    assert!(!rendered.contains("does not support cleanup lowering yet"));
    assert!(!rendered.contains("does not support `for await` lowering yet"));
}

#[test]
fn emits_for_await_lowering_for_projected_task_item_roots() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::TaskAwait,
        RuntimeCapability::AsyncIteration,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
use MORE as STATIC_ENV
use BOX as CONST_ENV

struct Pending {
tasks: [Task[Int]; 2],
}

extern "c" fn step(value: Int)

const BOX: Pending = Pending { tasks: [worker(1), worker(2)] }
static MORE: Pending = Pending { tasks: [worker(3), worker(4)] }

async fn worker(value: Int) -> Int {
return value
}

async fn main() -> Int {
var total = 0
for await value in BOX.tasks {
    total = total + value
}
for await value in STATIC_ENV.tasks {
    total = total + value
}
for await value in CONST_ENV.tasks {
    total = total + value
}
defer {
    for await value in BOX.tasks {
        step(value);
    }
    for await value in STATIC_ENV.tasks {
        step(value);
    }
    for await value in CONST_ENV.tasks {
        step(value);
    }
}
return total
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.contains("cleanup_for_cond"));
    assert!(rendered.matches("insertvalue").count() >= 3);
    assert!(rendered.matches("call ptr @qlrt_task_await").count() >= 6);
    assert!(rendered.matches("call void @step(i64").count() >= 1);
    assert!(!rendered.contains("does not support cleanup lowering yet"));
    assert!(!rendered.contains("does not support `for await` lowering yet"));
}

#[test]
fn emits_task_item_value_flow_through_locals_and_calls() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::TaskAwait,
        RuntimeCapability::AsyncIteration,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
use MORE as STATIC_ENV
use BOX as CONST_ENV

struct Pending {
tasks: [Task[Int]; 2],
}

extern "c" fn step(value: Int)

const BOX: Pending = Pending { tasks: [worker(1), worker(2)] }
static MORE: Pending = Pending { tasks: [worker(3), worker(4)] }

fn forward(env: Pending) -> Pending {
return env
}

async fn worker(value: Int) -> Int {
return value
}

async fn main() -> Int {
let branch = true
let pending = forward(if branch { BOX } else { STATIC_ENV })
let matched = forward(match branch {
    true => CONST_ENV,
    false => MORE,
})
var total = 0
for await value in pending.tasks {
    total = total + value
}
total = total + await matched.tasks[0]
defer {
    let cleanup_pending = forward(if branch { BOX } else { STATIC_ENV })
    for await value in cleanup_pending.tasks {
        step(value);
    }
    let cleanup_matched = forward(match branch {
        true => CONST_ENV,
        false => MORE,
    })
    step(await cleanup_matched.tasks[0]);
}
return total
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.contains("cleanup_for_cond"));
    assert!(rendered.contains("for_await_setup"));
    assert!(rendered.matches("call ptr @qlrt_task_await").count() >= 4);
    assert!(rendered.matches("call void @step(i64").count() >= 2);
    assert!(!rendered.contains("does not support cleanup lowering yet"));
    assert!(!rendered.contains("does not support `for await` lowering yet"));
}

#[test]
fn emits_cleanup_block_for_await_lowering_for_inline_task_roots() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::TaskAwait,
        RuntimeCapability::AsyncIteration,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
extern "c" fn step(value: Int)

async fn worker(value: Int) -> Int {
return value
}

async fn main() -> Int {
defer {
    for await value in [worker(1), worker(2)] {
        step(value);
    }
    for await item in (worker(3), worker(4)) {
        step(item);
    }
}
return 0
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.contains("cleanup_for_cond"));
    assert!(rendered.contains("cleanup_for_tuple_item"));
    assert!(rendered.matches("call ptr @ql_").count() >= 4);
    assert!(rendered.matches("call ptr @qlrt_task_await").count() >= 2);
    assert!(
        rendered
            .matches("call void @qlrt_task_result_release")
            .count()
            >= 2
    );
    assert!(!rendered.contains("does not support cleanup lowering yet"));
    assert!(!rendered.contains("does not support `for await` lowering yet"));
}

#[test]
fn emits_cleanup_block_for_await_lowering_for_awaited_projected_root() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::TaskAwait,
        RuntimeCapability::AsyncIteration,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
struct TaskArrayPayload {
tasks: [Task[Int]; 2],
}

struct TaskEnvelope {
payload: TaskArrayPayload,
}

extern "c" fn step(value: Int)

async fn worker(value: Int) -> Int {
return value
}

async fn task_env(base: Int) -> TaskEnvelope {
return TaskEnvelope {
    payload: TaskArrayPayload {
        tasks: [worker(base), worker(base + 1)],
    },
}
}

async fn main() -> Int {
defer {
    for await value in (await task_env(1)).payload.tasks {
        step(value);
    }
}
return 0
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.contains("cleanup_for_cond"));
    assert!(rendered.matches("call ptr @qlrt_task_await").count() >= 3);
    assert!(
        rendered
            .matches("call void @qlrt_task_result_release")
            .count()
            >= 3
    );
    assert!(!rendered.contains("does not support cleanup lowering yet"));
    assert!(!rendered.contains("does not support `for await` lowering yet"));
}

#[test]
fn emits_cleanup_block_let_struct_literal_with_awaited_projected_field() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::TaskAwait,
        RuntimeCapability::AsyncIteration,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
struct TaskArrayPayload {
tasks: [Task[Int]; 2],
}

struct TaskEnvelope {
payload: TaskArrayPayload,
}

struct Wrapper {
tasks: [Task[Int]; 2],
}

async fn worker(value: Int) -> Int {
return value
}

async fn task_env(base: Int) -> TaskEnvelope {
return TaskEnvelope {
    payload: TaskArrayPayload {
        tasks: [worker(base), worker(base + 1)],
    },
}
}

async fn main() -> Int {
defer {
    let wrapper = Wrapper { tasks: (await task_env(1)).payload.tasks }
    for await value in wrapper.tasks {
        let copy = value
    }
}
return 0
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.matches("insertvalue { [2 x ptr] }").count() >= 1);
    assert!(rendered.matches("call ptr @qlrt_task_await").count() >= 3);
    assert!(
        rendered
            .matches("call void @qlrt_task_result_release")
            .count()
            >= 3
    );
    assert!(!rendered.contains("does not support cleanup lowering yet"));
}

#[test]
fn emits_cleanup_block_for_await_lowering_for_projected_if_and_match_roots() {
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

extern "c" fn step(value: Int)

async fn worker(value: Int) -> Int {
return value
}

async fn main() -> Int {
let branch = true
defer {
    for await value in (if branch { Wrapper { tasks: [worker(1), worker(2)] } } else { Wrapper { tasks: [worker(3), worker(4)] } }).tasks {
        step(value);
    }
    for await item in (match branch { true => Wrapper { tasks: [worker(5), worker(6)] }, false => Wrapper { tasks: [worker(7), worker(8)] } }).tasks {
        step(item);
    }
}
return 0
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.contains("cleanup_then"));
    assert!(rendered.contains("cleanup_match_arm"));
    assert!(rendered.matches("call ptr @qlrt_task_await").count() >= 2);
    assert!(
        rendered
            .matches("call void @qlrt_task_result_release")
            .count()
            >= 2
    );
    assert!(!rendered.contains("does not support cleanup lowering yet"));
    assert!(!rendered.contains("does not support `for await` lowering yet"));
}

#[test]
fn emits_cleanup_block_for_await_lowering_for_projected_block_root() {
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
defer {
    for await value in ({ let wrapper = Wrapper { tasks: [worker(1), worker(2)] }; wrapper }).tasks {
        let copy = value
    }
}
return 0
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.contains("cleanup_for_cond"));
    assert!(rendered.matches("call ptr @qlrt_task_await").count() >= 1);
    assert!(
        rendered
            .matches("call void @qlrt_task_result_release")
            .count()
            >= 1
    );
    assert!(!rendered.contains("does not support cleanup lowering yet"));
    assert!(!rendered.contains("does not support `for await` lowering yet"));
}

#[test]
fn emits_cleanup_block_for_await_lowering_for_projected_assignment_root() {
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
var wrapper = Wrapper { tasks: [worker(0), worker(0)] }
defer {
    for await value in (wrapper = Wrapper { tasks: [worker(1), worker(2)] }).tasks {
        let copy = value
    }
}
return 0
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.contains("cleanup_for_cond"));
    assert!(rendered.matches("store { [2 x ptr] }").count() >= 2);
    assert!(rendered.matches("call ptr @qlrt_task_await").count() >= 1);
    assert!(
        rendered
            .matches("call void @qlrt_task_result_release")
            .count()
            >= 1
    );
    assert!(!rendered.contains("does not support cleanup lowering yet"));
    assert!(!rendered.contains("does not support `for await` lowering yet"));
}

#[test]
fn emits_cleanup_block_for_await_lowering_for_projected_question_root() {
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

fn helper() -> Wrapper {
return Wrapper { tasks: [worker(1), worker(2)] }
}

async fn main() -> Int {
defer {
    for await value in (helper()?).tasks {
        let copy = value
    }
}
return 0
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.contains("cleanup_for_cond"));
    assert!(rendered.matches("call ptr @qlrt_task_await").count() >= 1);
    assert!(
        rendered
            .matches("call void @qlrt_task_result_release")
            .count()
            >= 1
    );
    assert!(!rendered.contains("does not support cleanup lowering yet"));
    assert!(!rendered.contains("does not support `for await` lowering yet"));
}

#[test]
fn emits_cleanup_block_for_lowering_for_projected_question_root() {
    let rendered = emit(
        r#"
struct Boxed {
values: [Int; 3],
}

extern "c" fn sink(value: Int)

fn helper() -> Boxed {
return Boxed { values: [1, 2, 3] }
}

fn main() -> Int {
defer {
    for value in (helper()?).values {
        sink(value);
    }
}
return 0
}
"#,
    );

    assert!(rendered.contains("cleanup_for_cond"));
    assert!(rendered.contains("call void @sink(i64"));
    assert!(!rendered.contains("does not support cleanup lowering yet"));
    assert!(!rendered.contains("does not support `for` lowering yet"));
}

#[test]
fn emits_cleanup_block_for_destructuring() {
    let rendered = emit(
        r#"
struct Pair {
left: Int,
right: Int,
}

extern "c" fn sink(value: Int)

fn pair_values() -> [Pair; 2] {
return [Pair { left: 20, right: 22 }, Pair { left: 24, right: 26 }]
}

fn main() -> Int {
defer {
    for (first, _) in ((4, 6), (8, 10)) {
        sink(first);
    }
    for Pair { left, right: current } in pair_values() {
        sink(left);
        sink(current);
    }
}
return 0
}
"#,
    );

    assert!(rendered.contains("cleanup_for_cond"));
    assert!(rendered.contains("cleanup_for_tuple_item"));
    assert!(rendered.matches("extractvalue { i64, i64 }").count() >= 6);
    assert_eq!(rendered.matches("call void @sink(i64").count(), 4);
    assert!(!rendered.contains("does not support cleanup lowering yet"));
    assert!(!rendered.contains("does not support `for` lowering yet"));
}

#[test]
fn emits_cleanup_block_for_lowering_for_projected_and_call_roots() {
    let rendered = emit(
        r#"
struct Holder {
values: [Int; 3],
}

extern "c" fn sink(value: Int)

fn items() -> [Int; 3] {
return [4, 5, 6]
}

fn main() -> Int {
let holder = Holder { values: [1, 2, 3] }
defer {
    for value in holder.values {
        sink(value)
    }
    for item in items() {
        sink(item)
    }
}
return 0
}
"#,
    );

    assert!(rendered.contains("cleanup_for_cond"));
    assert!(rendered.contains("call i64 @ql_"));
    assert!(rendered.contains("call void @sink(i64"));
    assert!(!rendered.contains("does not support cleanup lowering yet"));
    assert!(!rendered.contains("does not support `for` lowering yet"));
}

#[test]
fn emits_cleanup_block_for_lowering_for_alias_and_nested_call_roots() {
    let rendered = emit(
        r#"
use items as load_items

struct Holder {
values: [Int; 3],
}

extern "c" fn sink(value: Int)

fn items() -> [Int; 3] {
return [4, 5, 6]
}

fn holder() -> Holder {
return Holder { values: [1, 2, 3] }
}

fn main() -> Int {
defer {
    for value in holder().values {
        sink(value)
    }
    for item in load_items() {
        sink(item)
    }
}
return 0
}
"#,
    );

    assert!(rendered.contains("cleanup_for_cond"));
    assert!(rendered.contains("call i64 @ql_"));
    assert!(rendered.contains("call void @sink(i64"));
    assert!(!rendered.contains("does not support cleanup lowering yet"));
    assert!(!rendered.contains("does not support `for` lowering yet"));
}

#[test]
fn emits_cleanup_block_for_lowering_for_const_and_static_roots() {
    let rendered = emit(
        r#"
use MORE as alias_more
use HOLDER as alias_holder

struct Holder {
values: [Int; 3],
}

extern "c" fn sink(value: Int)

const VALUES: [Int; 3] = [1, 2, 3]
static MORE: [Int; 3] = [4, 5, 6]
const HOLDER: Holder = Holder { values: [7, 8, 9] }

fn main() -> Int {
defer {
    for value in VALUES {
        sink(value)
    }
    for item in alias_more {
        sink(item)
    }
    for projected in alias_holder.values {
        sink(projected)
    }
}
return 0
}
"#,
    );

    assert!(rendered.contains("cleanup_for_cond"));
    assert!(rendered.contains("insertvalue [3 x i64]"));
    assert!(rendered.contains("call void @sink(i64"));
    assert!(!rendered.contains("does not support cleanup lowering yet"));
    assert!(!rendered.contains("does not support `for` lowering yet"));
}

#[test]
fn emits_cleanup_block_guard_scrutinee_and_value_lowering() {
    let rendered = emit(
        r#"
extern "c" fn note()
extern "c" fn first()
extern "c" fn second()
extern "c" fn sink(value: Int)

fn enabled() -> Bool {
return true
}

fn main() -> Int {
let flag = true
defer if {
    note();
    enabled()
} {
    match {
        note();
        flag
    } {
        true => sink({
            note();
            1
        }),
        false => second(),
    }
} else {
    first()
}
return 0
}
"#,
    );

    assert!(rendered.contains("call void @note()"));
    assert!(rendered.contains("call void @first()"));
    assert!(rendered.contains("call void @second()"));
    assert!(rendered.contains("call void @sink(i64 1)"));
    assert!(!rendered.contains("does not support cleanup lowering yet"));
    assert!(!rendered.contains("does not support `match` lowering yet"));
}

#[test]
fn emits_cleanup_lowering_for_foldable_control_flow_values() {
    let rendered = emit(
        r#"
struct Pair {
left: Int,
right: Int,
}

extern "c" fn first()
extern "c" fn second()
extern "c" fn sink(value: Int)

const PICK_LEFT: Bool = true
const PICK_VALUES: Int = 0

fn main() -> Int {
defer {
    let Pair { left, right } = if PICK_LEFT {
        Pair { left: 4, right: 6 }
    } else {
        Pair { left: 8, right: 10 }
    };
    sink(left);
    sink(right);
    for value in match PICK_VALUES {
        0 => [12, 14],
        _ => [16, 18],
    } {
        sink(value);
    }
    if match PICK_VALUES {
        0 => true,
        _ => false,
    } {
        first();
    } else {
        second();
    };
}
return 0
}
"#,
    );

    assert!(rendered.contains("cleanup_for_cond"));
    assert!(rendered.matches("extractvalue { i64, i64 }").count() >= 2);
    assert!(rendered.contains("insertvalue [2 x i64]"));
    assert!(rendered.contains("call void @first()"));
    assert!(!rendered.contains("does not support cleanup lowering yet"));
    assert!(!rendered.contains("does not support `for` lowering yet"));
    assert!(!rendered.contains("does not support `if` lowering yet"));
    assert!(!rendered.contains("does not support `match` lowering yet"));
}

#[test]
fn emits_match_question_mark_lowering() {
    let rendered = emit(
        r#"
fn enabled() -> Bool {
return false
}

fn helper() -> Int {
let flag = true
return match flag {
    true if enabled() => 1,
    false => 0,
}
}

fn main() -> Int {
return helper()?
}
"#,
    );

    assert!(rendered.contains("define i64 @ql_1_helper()"));
    assert!(rendered.contains("define i64 @ql_2_main()"));
    assert!(!rendered.contains("does not support `match` lowering yet"));
    assert!(!rendered.contains("does not support `?` lowering yet"));
}

#[test]
fn emits_cleanup_and_question_mark_lowering() {
    let rendered = emit(
        r#"
extern "c" fn first()

fn helper() -> Int {
return 1
}

fn main() -> Int {
defer first()
return helper()?
}
"#,
    );

    assert!(rendered.contains("call void @first()"));
    assert!(!rendered.contains("does not support cleanup lowering yet"));
    assert!(!rendered.contains("does not support `?` lowering yet"));
}

#[test]
fn emits_cleanup_internal_question_mark_lowering() {
    let rendered = emit(
        r#"
extern "c" fn first() -> Int

fn helper() -> Int {
return first()
}

fn main() -> Int {
defer helper()?
return 0
}
"#,
    );

    assert!(rendered.contains("call i64 @ql_1_helper()"));
    assert!(!rendered.contains("does not support cleanup lowering yet"));
    assert!(!rendered.contains("does not support `?` lowering yet"));
}

#[test]
fn dedupes_cleanup_and_capturing_closure_value_lowering_diagnostics() {
    let messages = emit_error(
        r#"
extern "c" fn first()

fn main() -> Int {
defer first()
let base = 1
let capture = move () => base
return capture()
}
"#,
    );

    assert_eq!(
        messages
            .iter()
            .filter(|message| {
                message.as_str()
                    == "LLVM IR backend foundation does not support cleanup lowering yet"
            })
            .count(),
        0
    );
    assert_eq!(
        messages
            .iter()
            .filter(|message| {
                message.as_str()
                    == "LLVM IR backend foundation currently only supports a narrow non-`move` capturing-closure subset: immutable same-function scalar, `String`, and task-handle captures through the currently shipped ordinary/control-flow and cleanup/guard-call roots"
            })
            .count(),
        1
    );
    assert!(messages.iter().all(|message| {
        !message.contains("could not resolve LLVM type for local")
            && !message.contains("could not infer LLVM type for MIR local")
    }));
}

#[test]
fn dedupes_cleanup_and_for_await_lowering_diagnostics() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::AsyncIteration,
    ]);
    let messages = emit_error_with_runtime_hooks(
        r#"
extern "c" fn first()

async fn helper() -> Int {
defer first()
for await value in 0 {
    break
}
return 0
}
"#,
        CodegenMode::Library,
        &runtime_hooks,
    );

    assert_eq!(
        messages
            .iter()
            .filter(|message| {
                message.as_str()
                    == "LLVM IR backend foundation does not support cleanup lowering yet"
            })
            .count(),
        0
    );
    assert_eq!(
        messages
            .iter()
            .filter(|message| {
                message.as_str()
                    == "LLVM IR backend foundation does not support `for await` lowering yet"
            })
            .count(),
        1
    );
    assert!(messages.iter().all(|message| {
        message != "LLVM IR backend foundation does not support `for` lowering yet"
            && message != "LLVM IR backend foundation does not support array values yet"
            && !message.contains("could not resolve LLVM type for local")
            && !message.contains("could not infer LLVM type for MIR local")
    }));
}
