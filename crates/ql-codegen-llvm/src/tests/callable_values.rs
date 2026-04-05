use super::*;

#[test]
fn emits_first_class_function_value_local_calls() {
    let rendered = emit(
        r#"
fn add_one(value: Int) -> Int {
return value + 1
}

fn main() -> Int {
let f = add_one
return f(41)
}
"#,
    );

    assert!(rendered.contains("store ptr @ql_0_add_one"));
    assert!(rendered.contains("load ptr, ptr %l1_f"));
    assert!(rendered.contains("call i64 %t0(i64 41)"));
}

#[test]
fn emits_async_function_value_local_calls() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
use worker as run_alias

async fn worker(value: Int) -> Int {
return value + 1
}

async fn main() -> Int {
let direct = worker
let aliased = run_alias
let first = await direct(10)
let second = await aliased(20)
return first + second
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.contains("store ptr @ql_0_worker, ptr %l1_direct"));
    assert!(rendered.contains("store ptr @ql_0_worker, ptr %l2_aliased"));
    assert!(rendered.contains("call ptr %t0(i64 10)"));
    assert!(rendered.contains("call ptr %t6(i64 20)"));
    assert!(rendered.contains("call ptr @qlrt_task_await"));
}

#[test]
fn emits_async_callable_const_and_static_item_calls() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
use APPLY_CONST as run_const
use APPLY_STATIC as run_static

async fn worker(value: Int) -> Int {
return value + 1
}

const APPLY_CONST: (Int) -> Task[Int] = worker
static APPLY_STATIC: (Int) -> Task[Int] = worker

async fn main() -> Int {
let f = run_const
let g = run_static
let first = await APPLY_CONST(10)
let second = await APPLY_STATIC(20)
let third = await f(30)
let fourth = await g(40)
return first + second + third + fourth
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.contains("store ptr @ql_0_worker, ptr %l1_f"));
    assert!(rendered.contains("store ptr @ql_0_worker, ptr %l2_g"));
    assert!(rendered.contains("call ptr @ql_0_worker(i64 10)"));
    assert!(rendered.contains("call ptr @ql_0_worker(i64 20)"));
    assert!(rendered.contains("load ptr, ptr %l1_f"));
    assert!(rendered.contains("load ptr, ptr %l2_g"));
    assert!(rendered.contains("i64 30)"));
    assert!(rendered.contains("i64 40)"));
    assert!(rendered.contains("call ptr @qlrt_task_await"));
}

#[test]
fn emits_awaited_guard_async_callable_control_flow_roots() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
use worker as async_alias
use APPLY as async_const_alias

async fn worker(value: Int) -> Int {
return value + 10
}

const APPLY: (Int) -> Task[Int] = worker

async fn main() -> Int {
let branch = true
return match 1 {
    1 if await (if branch { async_alias } else { async_const_alias })(3) == 13 => 10,
    1 if await (match branch { true => async_const_alias, false => async_alias })(4) == 14 => 20,
    _ => 0,
}
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.contains("call ptr @qlrt_task_await"));
    assert!(rendered.matches("call ptr %t").count() >= 2);
    assert!(rendered.contains("guard_match_arm"));
    assert!(!rendered.contains("does not support `match` lowering yet"));
    assert!(!rendered.contains("does not support imported value lowering yet"));
}

#[test]
fn emits_awaited_scrutinee_async_callable_control_flow_roots() {
    let runtime_hooks = collect_runtime_hook_signatures([
        RuntimeCapability::AsyncFunctionBodies,
        RuntimeCapability::TaskSpawn,
        RuntimeCapability::TaskAwait,
    ]);
    let rendered = emit_with_runtime_hooks(
        r#"
use worker as async_alias
use APPLY as async_const_alias

extern "c" fn sink(value: Int)

async fn worker(value: Int) -> Int {
return value + 10
}

const APPLY: (Int) -> Task[Int] = worker

async fn main() -> Int {
let branch = true
match await (if branch { async_alias } else { async_const_alias })(3) {
    13 => sink(1),
    _ => sink(0),
}
match await (match branch { true => async_const_alias, false => async_alias })(4) {
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
    assert!(!rendered.contains("does not support `match` lowering yet"));
    assert!(!rendered.contains("does not support imported value lowering yet"));
}

#[test]
fn emits_awaited_projection_async_callable_control_flow_scrutinees() {
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
match (await (if branch { async_alias } else { async_const_alias })(13)).value {
    13 => sink(1),
    _ => sink(0),
}
match (await (match branch { true => async_const_alias, false => async_alias })(14)).value {
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
    assert!(!rendered.contains("does not support `match` lowering yet"));
    assert!(!rendered.contains("does not support field projection lowering yet"));
}

#[test]
fn emits_awaited_aggregate_binding_scrutinees() {
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
match await (if branch { state_alias } else { state_const_alias })(13) {
    current => sink(current.slot.value),
}
match await (match branch { true => pair_const_alias, false => pair_alias })(20) {
    current => sink(current[0] + current[1]),
}
match await (if branch { values_alias } else { values_const_alias })(30) {
    current => sink(current[0] + current[2]),
}
return 0
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.matches("call ptr @qlrt_task_await").count() >= 2);
    assert!(rendered.contains("getelementptr inbounds { { i1, i64 } }"));
    assert!(rendered.contains("getelementptr inbounds { i64, i64 }"));
    assert!(rendered.contains("getelementptr inbounds [3 x i64]"));
    assert!(!rendered.contains("does not support `match` lowering yet"));
}

#[test]
fn emits_awaited_projection_async_callable_control_flow_guards() {
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

async fn load_state(value: Int) -> State {
return State {
    slot: Slot { value: value },
}
}

const LOAD: (Int) -> Task[State] = load_state

async fn main() -> Int {
let branch = true
return match 1 {
    1 if (await (if branch { async_alias } else { async_const_alias })(13)).slot.value == 13 => 10,
    1 if (await (match branch { true => async_const_alias, false => async_alias })(14)).slot.value == 14 => 20,
    _ => 0,
}
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.contains("call ptr @qlrt_task_await"));
    assert!(rendered.matches("call ptr %t").count() >= 2);
    assert!(rendered.contains("guard_match_arm"));
    assert!(!rendered.contains("does not support `match` lowering yet"));
    assert!(!rendered.contains("does not support field projection lowering yet"));
}

#[test]
fn emits_awaited_aggregate_guard_async_callable_control_flow_roots() {
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

async fn load_state(value: Int) -> State {
return State {
    slot: Slot { value: value },
}
}

fn matches(expected: Int, state: State) -> Bool {
return state.slot.value == expected
}

const LOAD: (Int) -> Task[State] = load_state

async fn main() -> Int {
let branch = true
return match 1 {
    1 if matches(13, await (if branch { async_alias } else { async_const_alias })(13)) => 10,
    1 if matches(14, await (match branch { true => async_const_alias, false => async_alias })(14)) => 20,
    _ => 0,
}
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.contains("call ptr @qlrt_task_await"));
    assert!(rendered.matches("call ptr %t").count() >= 2);
    assert!(rendered.contains("call i1 @ql_3_matches"));
    assert!(rendered.contains("guard_match_arm"));
    assert!(!rendered.contains("does not support `match` lowering yet"));
    assert!(!rendered.contains("does not support imported value lowering yet"));
}

#[test]
fn emits_awaited_call_backed_aggregate_guard_async_callable_control_flow_roots() {
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

async fn load_state(value: Int) -> State {
return State {
    slot: Slot { value: value },
}
}

fn matches(expected: Int, state: State) -> Bool {
return state.slot.value == expected
}

fn wraps_match(expected: Int, state: State) -> Bool {
return matches(expected, state)
}

const LOAD: (Int) -> Task[State] = load_state

async fn main() -> Int {
let branch = true
return match 1 {
    1 if wraps_match(13, await (if branch { async_alias } else { async_const_alias })(13)) => 10,
    1 if wraps_match(14, await (match branch { true => async_const_alias, false => async_alias })(14)) => 20,
    _ => 0,
}
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.contains("call ptr @qlrt_task_await"));
    assert!(rendered.matches("call ptr %t").count() >= 2);
    assert!(rendered.contains("call i1 @ql_4_wraps_match"));
    assert!(rendered.contains("call i1 @ql_3_matches"));
    assert!(rendered.contains("guard_match_arm"));
    assert!(!rendered.contains("does not support `match` lowering yet"));
    assert!(!rendered.contains("does not support imported value lowering yet"));
}

#[test]
fn emits_awaited_guard_import_alias_helpers() {
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

struct Slot {
value: Int,
}

struct State {
slot: Slot,
}

async fn load_state(value: Int) -> State {
return State {
    slot: Slot { value: value },
}
}

fn matches(expected: Int, state: State) -> Bool {
return state.slot.value == expected
}

const LOAD: (Int) -> Task[State] = load_state

async fn main() -> Int {
let branch = true
return match 1 {
    1 if helper_alias(13, await (if branch { async_alias } else { async_const_alias })(13)) => 10,
    1 if helper_alias(14, await (match branch { true => async_const_alias, false => async_alias })(14)) => 20,
    _ => 0,
}
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.contains("call ptr @qlrt_task_await"));
    assert!(rendered.matches("call ptr %t").count() >= 2);
    assert!(rendered.contains("call i1 @ql_3_matches"));
    assert!(rendered.contains("guard_match_arm"));
    assert!(!rendered.contains("does not support `match` lowering yet"));
    assert!(!rendered.contains("does not support imported value lowering yet"));
}

#[test]
fn emits_awaited_nested_call_root_runtime_projection_guards() {
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
return match 1 {
    1 if wrap(await (if branch { async_alias } else { async_const_alias })(13)).slot.value == 13 => 10,
    1 if wrap(await (match branch { true => async_const_alias, false => async_alias })(14)).slot.value == 14 => 20,
    1 if matches(value: [wrap(await (if branch { async_alias } else { async_const_alias })(15)).slot.value, 0][offset(11)], expected: 15) => 30,
    _ => 0,
}
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.matches("call ptr @qlrt_task_await").count() >= 3);
    assert!(rendered.matches("call ptr %t").count() >= 3);
    assert!(rendered.matches("call i1 @ql_").count() >= 1);
    assert!(rendered.contains("guard_match_arm"));
    assert!(!rendered.contains("does not support `match` lowering yet"));
    assert!(!rendered.contains("does not support field projection lowering yet"));
}

#[test]
fn emits_awaited_inline_guard_families() {
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

async fn load_state(value: Int) -> State {
return State {
    value: value,
}
}

fn matches(pair: (Int, Int), expected: Int) -> Bool {
return pair[1] == expected
}

fn contains(values: [Int; 3], expected: Int) -> Bool {
return values[1] == expected
}

const LOAD: (Int) -> Task[State] = load_state

async fn main() -> Int {
let branch = true
return match 1 {
    1 if State { value: (await (if branch { async_alias } else { async_const_alias })(22)).value }.value == 22 => 10,
    1 if matches((0, (await (match branch { true => async_const_alias, false => async_alias })(23)).value), 23) => 20,
    1 if contains([0, (await (if branch { async_alias } else { async_const_alias })(24)).value, 2], 24) => 30,
    _ => 0,
}
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.matches("call ptr @qlrt_task_await").count() >= 3);
    assert!(rendered.matches("call ptr %t").count() >= 3);
    assert!(rendered.matches("call i1 @ql_").count() >= 2);
    assert!(rendered.contains("guard_match_arm"));
    assert!(!rendered.contains("does not support `match` lowering yet"));
    assert!(!rendered.contains("does not support field projection lowering yet"));
}

#[test]
fn emits_awaited_scrutinee_families() {
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

fn matches(expected: Int, value: Int) -> Bool {
return expected == value
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
match helper_alias(13, (await (if branch { async_alias } else { async_const_alias })(13)).slot.value) {
    true => sink(1),
    false => sink(0),
}
match State { slot: Slot { value: (await (match branch { true => async_const_alias, false => async_alias })(14)).slot.value } }.slot.value {
    14 => sink(2),
    _ => sink(3),
}
match wrap(await (if branch { async_alias } else { async_const_alias })(15)).slot.value {
    15 => sink(4),
    _ => sink(5),
}
match [wrap(await (match branch { true => async_const_alias, false => async_alias })(17)).slot.value, 0][offset(11)] {
    17 => sink(6),
    _ => sink(7),
}
return 0
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.matches("call ptr @qlrt_task_await").count() >= 4);
    assert!(rendered.matches("call ptr %t").count() >= 4);
    assert!(rendered.contains("call i1 @ql_4_matches"));
    assert!(!rendered.contains("does not support `match` lowering yet"));
    assert!(!rendered.contains("does not support field projection lowering yet"));
}

#[test]
fn emits_non_capturing_closure_value_local_calls() {
    let rendered = emit(
        r#"
fn main() -> Int {
let run = () => 41
return run()
}
"#,
    );

    assert!(rendered.contains("store ptr @ql_0_main__closure0"));
    assert!(rendered.contains("load ptr, ptr %l2_run"));
    assert!(rendered.contains("call i64 %t1()"));
    assert!(rendered.contains("define i64 @ql_0_main__closure0()"));
}

#[test]
fn emits_parameterized_non_capturing_closure_value_local_calls() {
    let rendered = emit(
        r#"
fn main() -> Int {
let run = (value) => value + 1
let alias = run
return alias(41)
}
"#,
    );

    assert!(rendered.contains("store ptr @ql_0_main__closure0"));
    assert!(rendered.contains("load ptr, ptr %l3_alias"));
    assert!(rendered.contains("call i64 %t2(i64 41)"));
    assert!(rendered.contains("define i64 @ql_0_main__closure0(i64 %arg0)"));
}

#[test]
fn emits_typed_non_capturing_closure_value_local_calls() {
    let rendered = emit(
        r#"
fn main() -> Int {
let run = (value: Int) => value + 1
let alias = run
return alias(41)
}
"#,
    );

    assert!(rendered.contains("store ptr @ql_0_main__closure0"));
    assert!(rendered.contains("load ptr, ptr %l3_alias"));
    assert!(rendered.contains("call i64 %t2(i64 41)"));
    assert!(rendered.contains("define i64 @ql_0_main__closure0(i64 %arg0)"));
}

#[test]
fn emits_annotated_local_non_capturing_closure_value_local_calls() {
    let rendered = emit(
        r#"
fn main() -> Int {
let run: (Int) -> Int = (value) => value + 1
let alias = run
return alias(41)
}
"#,
    );

    assert!(rendered.contains("store ptr @ql_0_main__closure0"));
    assert!(rendered.contains("load ptr, ptr %l3_alias"));
    assert!(rendered.contains("call i64 %t2(i64 41)"));
    assert!(rendered.contains("define i64 @ql_0_main__closure0(i64 %arg0)"));
}

#[test]
fn emits_direct_local_capturing_closure_calls() {
    let rendered = emit(
        r#"
fn main() -> Int {
let base = 41
let run = () => base + 1
return run()
}
"#,
    );

    assert!(rendered.contains("store ptr null"));
    assert!(rendered.contains("define i64 @ql_0_main__closure0(i64 %arg0)"));
    assert!(rendered.contains("call i64 @ql_0_main__closure0("));
    assert!(!rendered.contains("load ptr, ptr %l2_run"));
    assert!(
        !rendered.contains("currently only supports a narrow non-`move` capturing-closure subset")
    );
}

#[test]
fn emits_immutable_alias_direct_local_capturing_closure_calls() {
    let rendered = emit(
        r#"
fn main() -> Int {
let base = 41
let capture = () => base + 1
let alias = capture
return alias()
}
"#,
    );

    assert!(rendered.contains("store ptr null"));
    assert!(rendered.contains("define i64 @ql_0_main__closure0(i64 %arg0)"));
    assert!(rendered.contains("call i64 @ql_0_main__closure0("));
    assert!(
        !rendered.contains("currently only supports a narrow non-`move` capturing-closure subset")
    );
}

#[test]
fn emits_mutable_alias_direct_local_capturing_closure_calls() {
    let rendered = emit(
        r#"
fn main() -> Int {
let base = 41
let capture = () => base + 1
var alias = capture
return alias()
}
"#,
    );

    assert!(rendered.contains("store ptr null"));
    assert!(rendered.contains("define i64 @ql_0_main__closure0(i64 %arg0)"));
    assert!(rendered.contains("call i64 @ql_0_main__closure0("));
    assert!(
        !rendered.contains("currently only supports a narrow non-`move` capturing-closure subset")
    );
}

#[test]
fn emits_same_target_reassigned_mutable_alias_capturing_closure_calls() {
    let rendered = emit(
        r#"
fn main() -> Int {
let base = 41
let capture = () => base + 1
var alias = capture
alias = capture
return alias()
}
"#,
    );

    assert!(rendered.contains("store ptr null"));
    assert!(rendered.contains("define i64 @ql_0_main__closure0(i64 %arg0)"));
    assert!(rendered.contains("call i64 @ql_0_main__closure0("));
    assert!(
        !rendered.contains("currently only supports a narrow non-`move` capturing-closure subset")
    );
}

#[test]
fn emits_same_target_control_flow_capturing_closure_calls() {
    let rendered = emit(
        r#"
fn main() -> Int {
let branch = true
let value = 1
let capture = () => value
let alias = capture
let chosen_if = if branch { capture } else { alias }
let chosen_match = match branch {
    true => alias,
    false => capture,
}
return chosen_if() + chosen_match()
}
"#,
    );

    assert!(rendered.matches("@ql_0_main__closure0(").count() >= 2);
    assert!(
        !rendered.contains("currently only supports a narrow non-`move` capturing-closure subset")
    );
}

#[test]
fn emits_extended_ordinary_capturing_closure_call_roots() {
    let rendered = emit(
        r#"
fn main() -> Int {
let branch = true
let target = 42
let run = (value: Int) => value + target
var alias = run
let direct_assignment = (alias = run)(1)
let direct_control_flow_assignment = (if branch { alias = run } else { run })(2)
let direct_block_local = (match branch {
    true => {
        var local = run
        local = run;
        local
    },
    false => run,
})(3)
let chosen = if branch {
    let local = run
    local
} else {
    run
}
return direct_assignment + direct_control_flow_assignment + direct_block_local + chosen(4)
}
"#,
    );

    assert!(rendered.matches("@ql_0_main__closure0(").count() >= 4);
    assert!(rendered.contains("call i64 @ql_0_main__closure0("));
    assert!(
        !rendered.contains("currently only supports a narrow non-`move` capturing-closure subset")
    );
}

#[test]
fn emits_callable_const_and_static_item_calls() {
    let rendered = emit(
        r#"
use APPLY_CONST as run_const
use APPLY_STATIC as run_static

fn add_one(value: Int) -> Int {
return value + 1
}

const APPLY_CONST: (Int) -> Int = add_one
static APPLY_STATIC: (Int) -> Int = add_one

fn main() -> Int {
let f = run_const
let g = run_static
return APPLY_CONST(10) + APPLY_STATIC(20) + f(30) + g(40)
}
"#,
    );

    assert!(rendered.contains("call i64 @ql_0_add_one(i64 10)"));
    assert!(rendered.contains("call i64 @ql_0_add_one(i64 20)"));
    assert!(rendered.contains("store ptr @ql_0_add_one"));
    assert!(rendered.contains("load ptr, ptr %l1_f"));
    assert!(rendered.contains("load ptr, ptr %l2_g"));
    assert!(!rendered.contains("does not support callable const/static values yet"));
    assert!(!rendered.contains("does not support imported value lowering yet"));
}

#[test]
fn emits_cleanup_callable_const_alias_calls() {
    let rendered = emit(
        r#"
use APPLY as run

fn add_one(value: Int) -> Int {
return value + 1
}

const APPLY: (Int) -> Int = add_one

fn main() -> Int {
defer run(41)
return 0
}
"#,
    );

    assert!(rendered.contains("define i64 @ql_0_add_one(i64 %arg0)"));
    assert!(rendered.contains("store ptr @ql_0_add_one"));
    assert!(rendered.contains("call i64 %t"));
    assert!(rendered.contains("(i64 41)"));
    assert!(!rendered.contains("does not support cleanup lowering yet"));
}

#[test]
fn emits_foldable_function_item_cleanup_and_guard_calls() {
    let rendered = emit(
        r#"
use first as first_alias
use second as second_alias
use ready as ready_alias
use idle as idle_alias

extern "c" fn first()
extern "c" fn second()

fn ready() -> Bool {
return true
}

fn idle() -> Bool {
return false
}

const PICK_BOOL: Bool = true
const PICK_INT: Int = 0

fn main() -> Int {
defer (if PICK_BOOL {
    first
} else {
    second
})()
defer (match PICK_INT {
    0 => first_alias,
    _ => second_alias,
})()
defer if (if PICK_BOOL {
    ready_alias
} else {
    idle_alias
})() {
    first()
} else {
    second()
}
return 0
}
"#,
    );

    assert!(rendered.matches("call void %t").count() >= 2);
    assert!(rendered.contains("call i1 %t"));
    assert!(rendered.contains("store ptr @first"));
    assert!(!rendered.contains("does not support cleanup lowering yet"));
    assert!(!rendered.contains("does not support imported value lowering yet"));
}

#[test]
fn emits_closure_backed_callable_const_and_static_item_calls() {
    let rendered = emit(
        r#"
use APPLY_CONST as run_const
use APPLY_STATIC as run_static

const APPLY_CONST: (Int) -> Int = (value: Int) => value + 1
static APPLY_STATIC: (Int) -> Int = (value: Int) => value + 2

fn main() -> Int {
let f = run_const
let g = run_static
return APPLY_CONST(10) + APPLY_STATIC(20) + f(30) + g(40)
}
"#,
    );

    assert!(rendered.contains("store ptr @ql_0_APPLY_CONST__closure0"));
    assert!(rendered.contains("store ptr @ql_1_APPLY_STATIC__closure0"));
    assert!(rendered.contains("call i64 @ql_0_APPLY_CONST__closure0(i64 10)"));
    assert!(rendered.contains("call i64 @ql_1_APPLY_STATIC__closure0(i64 20)"));
    assert!(rendered.contains("load ptr, ptr %l1_f"));
    assert!(rendered.contains("load ptr, ptr %l2_g"));
    assert!(!rendered.contains("does not support callable const/static values yet"));
}

#[test]
fn emits_closure_backed_callable_globals_in_cleanup_and_match_guards() {
    let rendered = emit(
        r#"
const CHECK: (Int) -> Bool = (value: Int) => value == 42
static APPLY: (Int) -> Int = (value: Int) => value + 1

fn main() -> Int {
defer APPLY(41)
return match 42 {
    current if CHECK(current) => 1,
    _ => 0,
}
}
"#,
    );

    assert!(rendered.contains("define i1 @ql_0_CHECK__closure0(i64 %arg0)"));
    assert!(rendered.contains("define i64 @ql_1_APPLY__closure0(i64 %arg0)"));
    assert!(rendered.contains("call i1 %t"));
    assert!(rendered.contains("call i64 %t"));
    assert!(!rendered.contains("does not support cleanup lowering yet"));
    assert!(!rendered.contains("does not support callable const/static values yet"));
    assert!(!rendered.contains("does not support `match` lowering yet"));
}

#[test]
fn emits_local_non_capturing_closures_in_cleanup_and_match_guards() {
    let rendered = emit(
        r#"
fn main() -> Int {
let check = (value: Int) => value == 42
let run = (value: Int) => value + 1
defer run(41)
return match 42 {
    current if check(current) => 1,
    _ => 0,
}
}
"#,
    );

    assert!(rendered.contains("define i1 @ql_0_main__closure0(i64 %arg0)"));
    assert!(rendered.contains("define i64 @ql_0_main__closure1(i64 %arg0)"));
    assert!(rendered.contains("call i1 %t"));
    assert!(rendered.contains("call i64 %t"));
    assert!(!rendered.contains("does not support cleanup lowering yet"));
    assert!(!rendered.contains("does not support `match` lowering yet"));
    assert!(!rendered.contains("currently only supports non-capturing sync closure values"));
}

#[test]
fn emits_local_capturing_closures_in_cleanup_and_match_guards() {
    let rendered = emit(
        r#"
fn main() -> Int {
let target = 42
let check = (value: Int) => value == target
let base = 40
let run = (value: Int) => value + base + 1
defer run(1)
return match 42 {
    current if check(current) => 1,
    _ => 0,
}
}
"#,
    );

    assert!(rendered.contains("call i1 @ql_0_main__closure0("));
    assert!(rendered.contains("call i64 @ql_0_main__closure1("));
    assert!(!rendered.contains("does not support cleanup lowering yet"));
    assert!(!rendered.contains("does not support `match` lowering yet"));
    assert!(!rendered.contains(
        "currently only supports direct local calls for non-`move` closures that capture immutable same-function scalar bindings"
    ));
}

#[test]
fn emits_local_capturing_closures_in_cleanup_if_and_match_guards() {
    let rendered = emit(
        r#"
extern "c" fn keep()

fn main() -> Int {
let target = 42
let check = (value: Int) => value == target
defer if check(42) {
    keep()
}
defer match 42 {
    current if check(current) => keep(),
    _ => keep(),
}
return 0
}
"#,
    );

    assert!(rendered.matches("__closure0(").count() >= 3);
    assert!(!rendered.contains("does not support cleanup lowering yet"));
    assert!(!rendered.contains(
        "currently only supports direct local calls for non-`move` closures that capture immutable same-function scalar bindings"
    ));
}

#[test]
fn emits_cleanup_block_local_capturing_closure_alias_calls() {
    let rendered = emit(
        r#"
extern "c" fn keep()

fn main() -> Int {
let target = 42
let check = (value: Int) => value == target
let base = 40
let run = (value: Int) => value + base + 1
defer {
    let alias_run = run
    let alias_check = check
    alias_run(1)
    if alias_check(42) {
        keep()
    }
}
return 0
}
"#,
    );

    assert!(rendered.matches("__closure").count() >= 2);
    assert!(rendered.contains("call i64 @"));
    assert!(rendered.contains("call i1 @"));
    assert!(!rendered.contains("does not support cleanup lowering yet"));
    assert!(
        !rendered.contains("currently only supports a narrow non-`move` capturing-closure subset")
    );
}

#[test]
fn emits_cleanup_block_local_mutable_capturing_closure_reassign_calls() {
    let rendered = emit(
        r#"
extern "c" fn keep()

fn main() -> Int {
let target = 42
let check = (value: Int) => value == target
let base = 40
let run = (value: Int) => value + base + 1
defer {
    var alias_run = run
    alias_run = run;
    alias_run(1)
    var alias_check = check
    alias_check = check;
    if alias_check(42) {
        keep()
    }
}
return 0
}
"#,
    );

    assert!(rendered.matches("__closure").count() >= 2);
    assert!(rendered.contains("call i64 @"));
    assert!(rendered.contains("call i1 @"));
    assert!(!rendered.contains("does not support cleanup lowering yet"));
    assert!(
        !rendered.contains("currently only supports a narrow non-`move` capturing-closure subset")
    );
}

#[test]
fn emits_cleanup_assignment_valued_capturing_closure_calls() {
    let rendered = emit(
        r#"
extern "c" fn keep()

fn main() -> Int {
let target = 42
let check = (value: Int) => value == target
var check_alias = check
let run = (value: Int) => value + target
var run_alias = run
defer (run_alias = run)(1)
defer if (check_alias = check)(42) {
    keep()
}
return 0
}
"#,
    );

    assert!(rendered.matches("__closure").count() >= 2);
    assert!(rendered.contains("call i64 @"));
    assert!(rendered.contains("call i1 @"));
    assert!(!rendered.contains("does not support cleanup lowering yet"));
    assert!(
        !rendered.contains("currently only supports a narrow non-`move` capturing-closure subset")
    );
}

#[test]
fn emits_cleanup_control_flow_assignment_valued_capturing_closure_calls() {
    let rendered = emit(
        r#"
extern "c" fn keep()

fn main() -> Int {
let branch = true
let target = 42
let check = (value: Int) => value == target
var check_alias = check
let run = (value: Int) => value + target
var run_alias = run
defer (if branch { run_alias = run } else { run })(1)
defer if (match branch {
    true => check_alias = check,
    false => check,
})(42) {
    keep()
}
return 0
}
"#,
    );

    assert!(rendered.matches("__closure").count() >= 2);
    assert!(rendered.contains("call i64 %t"));
    assert!(rendered.contains("call i1 %t"));
    assert!(!rendered.contains("does not support cleanup lowering yet"));
    assert!(
        !rendered.contains("currently only supports a narrow non-`move` capturing-closure subset")
    );
}

#[test]
fn emits_cleanup_block_assignment_valued_capturing_closure_bindings() {
    let rendered = emit(
        r#"
extern "c" fn keep()

fn main() -> Int {
let target = 42
let check = (value: Int) => value == target
var check_alias = check
let run = (value: Int) => value + target
var run_alias = run
defer {
    let chosen_run = run_alias = run
    let chosen_check = check_alias = check
    chosen_run(1)
    if chosen_check(42) {
        keep()
    }
}
return 0
}
"#,
    );

    assert!(rendered.matches("__closure").count() >= 2);
    assert!(rendered.contains("call i64 @"));
    assert!(rendered.contains("call i1 @"));
    assert!(!rendered.contains("does not support cleanup lowering yet"));
    assert!(
        !rendered.contains("currently only supports a narrow non-`move` capturing-closure subset")
    );
}

#[test]
fn emits_cleanup_block_control_flow_assignment_valued_capturing_closure_bindings() {
    let rendered = emit(
        r#"
extern "c" fn keep()

fn main() -> Int {
let branch = true
let target = 42
let check = (value: Int) => value == target
var check_alias = check
let run = (value: Int) => value + target
var run_alias = run
defer {
    let chosen_run = if branch { run_alias = run } else { run }
    let chosen_check = match branch {
        true => check_alias = check,
        false => check,
    }
    chosen_run(1)
    if chosen_check(42) {
        keep()
    }
}
return 0
}
"#,
    );

    assert!(rendered.matches("__closure").count() >= 2);
    assert!(rendered.contains("call i64 @"));
    assert!(rendered.contains("call i1 @"));
    assert!(rendered.contains("br i1"));
    assert!(!rendered.contains("does not support cleanup lowering yet"));
    assert!(
        !rendered.contains("currently only supports a narrow non-`move` capturing-closure subset")
    );
}

#[test]
fn emits_cleanup_block_control_flow_local_alias_capturing_closure_bindings() {
    let rendered = emit(
        r#"
extern "c" fn keep()

fn main() -> Int {
let branch = true
let target = 42
let check = (value: Int) => value == target
let run = (value: Int) => value + target
defer {
    let chosen_run = if branch {
        let alias = run
        alias
    } else {
        run
    }
    let chosen_check = match branch {
        true => {
            var alias = check
            alias = check;
            alias
        },
        false => check,
    }
    chosen_run(1)
    if chosen_check(42) {
        keep()
    }
}
return 0
}
"#,
    );

    assert!(rendered.matches("__closure").count() >= 2);
    assert!(rendered.contains("call i64 @"));
    assert!(rendered.contains("call i1 @"));
    assert!(rendered.contains("br i1"));
    assert!(!rendered.contains("does not support cleanup lowering yet"));
    assert!(
        !rendered.contains("currently only supports a narrow non-`move` capturing-closure subset")
    );
}

#[test]
fn emits_cleanup_control_flow_local_alias_capturing_closure_call_roots() {
    let rendered = emit(
        r#"
extern "c" fn keep()

fn main() -> Int {
let branch = true
let target = 42
let check = (value: Int) => value == target
let run = (value: Int) => value + target
defer (if branch {
    let alias = run
    alias
} else {
    run
})(1)
defer if (match branch {
    true => {
        var alias = check
        alias = check;
        alias
    },
    false => check,
})(42) {
    keep()
}
return 0
}
"#,
    );

    assert!(rendered.matches("__closure").count() >= 2);
    assert!(rendered.contains("call i64 @"));
    assert!(rendered.contains("call i1 @"));
    assert!(rendered.contains("br i1"));
    assert!(!rendered.contains("does not support cleanup lowering yet"));
    assert!(
        !rendered.contains("currently only supports a narrow non-`move` capturing-closure subset")
    );
}

#[test]
fn emits_cleanup_different_closure_capturing_closure_call_roots() {
    let rendered = emit(
        r#"
extern "c" fn keep()

fn main() -> Int {
let branch = true
let target = 42
let left_run = (value: Int) => value + target
let right_run = (value: Int) => value + target + 1
let left_check = (value: Int) => value == target
let right_check = (value: Int) => value + 1 == target + 1
defer (if branch { left_run } else { right_run })(1)
defer (match branch {
    true => {
        let alias = left_run
        alias
    },
    false => right_run,
})(2)
defer if (if branch { left_check } else { right_check })(42) {
    keep()
}
defer if (match branch {
    true => {
        let alias = left_check
        alias
    },
    false => right_check,
})(42) {
    keep()
}
return 0
}
"#,
    );

    assert!(rendered.matches("__closure").count() >= 4);
    assert!(rendered.contains("cleanup_call_if_then"));
    assert!(rendered.contains("cleanup_call_match_arm"));
    assert!(rendered.contains("guard_call_if_then"));
    assert!(rendered.contains("guard_call_match_arm"));
    assert!(rendered.contains("call i64 @"));
    assert!(rendered.contains("call i1 @"));
    assert!(!rendered.contains("does not support cleanup lowering yet"));
    assert!(
        !rendered.contains("currently only supports a narrow non-`move` capturing-closure subset")
    );
}

#[test]
fn emits_match_guard_control_flow_local_alias_capturing_closure_call_roots() {
    let rendered = emit(
        r#"
fn main() -> Int {
let branch = true
let target = 42
let check = (value: Int) => value == target
let first = match 42 {
    current if (match branch {
        true => {
            let alias = check
            alias
        },
        false => check,
    })(current) => 1,
    _ => 0,
}
let second = match 42 {
    current if (if branch {
        var alias = check
        alias = check;
        alias
    } else {
        check
    })(current) => 2,
    _ => 0,
}
return first + second
}
"#,
    );

    assert!(rendered.matches("__closure0(").count() >= 3);
    assert!(rendered.contains("call i1 @"));
    assert!(!rendered.contains("does not support `match` lowering yet"));
    assert!(
        !rendered.contains("currently only supports a narrow non-`move` capturing-closure subset")
    );
}

#[test]
fn emits_match_guard_bound_control_flow_capturing_closure_calls() {
    let rendered = emit(
        r#"
fn main() -> Int {
let branch = true
let target = 42
let check = (value: Int) => value == target
var alias = check
let chosen = if branch {
    let local = check
    local
} else {
    check
}
let rebound = match branch {
    true => alias = check,
    false => check,
}
let first = match 42 {
    current if chosen(current) => 1,
    _ => 0,
}
let second = match 42 {
    current if rebound(current) => 2,
    _ => 0,
}
return first + second
}
"#,
    );

    assert!(rendered.matches("__closure0(").count() >= 3);
    assert!(rendered.contains("call i1 @"));
    assert!(!rendered.contains("does not support `match` lowering yet"));
    assert!(
        !rendered.contains("currently only supports a narrow non-`move` capturing-closure subset")
    );
}

#[test]
fn emits_match_guard_block_assignment_bound_capturing_closure_calls() {
    let rendered = emit(
        r#"
fn main() -> Int {
let branch = true
let target = 42
let check = (value: Int) => value == target
let first = match 42 {
    current if ({
        var alias = check
        let chosen = alias = check
        chosen
    })(current) => 1,
    _ => 0,
}
let second = match 42 {
    current if ({
        var alias = check
        let chosen = if branch { alias = check } else { check }
        chosen
    })(current) => 2,
    _ => 0,
}
return first + second
}
"#,
    );

    assert!(rendered.matches("__closure0(").count() >= 3);
    assert!(rendered.contains("call i1 @"));
    assert!(!rendered.contains("does not support `match` lowering yet"));
    assert!(
        !rendered.contains("currently only supports a narrow non-`move` capturing-closure subset")
    );
}

#[test]
fn emits_match_guard_different_closure_block_alias_capturing_closure_calls() {
    let rendered = emit(
        r#"
fn main() -> Int {
let branch = true
let target = 42
let left = (value: Int) => value == target
let right = (value: Int) => value + 1 == target + 1
let first = match 42 {
    current if (if branch {
        let alias = left
        alias
    } else {
        right
    })(current) => 1,
    _ => 0,
}
let second = match 42 {
    current if (match branch {
        true => {
            var alias = left
            alias = left;
            alias
        },
        false => right,
    })(current) => 2,
    _ => 0,
}
return first + second
}
"#,
    );

    assert!(rendered.matches("__closure").count() >= 2);
    assert!(rendered.contains("guard_call_if_then"));
    assert!(rendered.contains("guard_call_match_arm"));
    assert!(rendered.contains("call i1 @"));
    assert!(!rendered.contains("does not support `match` lowering yet"));
    assert!(
        !rendered.contains("currently only supports a narrow non-`move` capturing-closure subset")
    );
}

#[test]
fn emits_ordinary_different_closure_control_flow_capturing_closure_calls() {
    let rendered = emit(
        r#"
fn main() -> Int {
let branch = true
let target = 42
let left = (value: Int) => value + target
let right = (value: Int) => value + target + 1
return (if branch { left } else { right })(1)
    + (match branch {
        true => {
            let alias = left
            alias
        },
        false => right,
    })(2)
}
"#,
    );

    assert!(rendered.contains("ordinary_call_if_then"));
    assert!(rendered.contains("@ql_0_main__closure0("));
    assert!(rendered.contains("@ql_0_main__closure1("));
    assert!(
        !rendered.contains("currently only supports a narrow non-`move` capturing-closure subset")
    );
}

#[test]
fn emits_ordinary_different_closure_control_flow_binding_root_calls() {
    let rendered = emit(
        r#"
fn main() -> Int {
let branch = true
let target = 42
let left = (value: Int) => value + target
let right = (value: Int) => value + target + 1
let chosen_if = if branch { left } else { right }
let chosen_match = match branch {
    true => {
        let alias = left
        alias
    },
    false => right,
}
let alias_if = chosen_if
let alias_match = chosen_match
return alias_if(1) + alias_match(2)
}
"#,
    );

    assert!(rendered.matches("ordinary_call_if_then").count() >= 2);
    assert!(rendered.contains("@ql_0_main__closure0("));
    assert!(rendered.contains("@ql_0_main__closure1("));
    assert!(
        !rendered.contains("currently only supports a narrow non-`move` capturing-closure subset")
    );
}
