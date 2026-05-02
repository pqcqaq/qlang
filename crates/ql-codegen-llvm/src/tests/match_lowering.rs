use super::*;

#[test]
fn emits_bool_match_lowering() {
    let rendered = emit_with_mode(
        r#"
fn main() -> Int {
let flag = true
return match flag {
    true => 1,
    false => 0,
}
}
"#,
        CodegenMode::Program,
    );

    assert!(rendered.contains("br i1"));
    assert!(!rendered.contains("does not support `match` lowering yet"));
}

#[test]
fn emits_bool_match_with_binding_catch_all_lowering() {
    let rendered = emit_with_mode(
        r#"
fn main() -> Int {
let flag = false
return match flag {
    true => 1,
    other => if other { 2 } else { 0 },
}
}
"#,
        CodegenMode::Program,
    );

    assert!(rendered.contains("%l4_other = alloca i1"));
    assert!(rendered.contains("load i1, ptr %l4_other"));
    assert!(!rendered.contains("does not support `match` lowering yet"));
}

#[test]
fn emits_generic_enum_match_after_local_alias_lowering() {
    let rendered = emit(
        r#"
enum Option[T] {
    Some(T),
    None,
}

fn choose(value: Option[Int]) -> Int {
let kept: Option[Int] = value
return match kept {
    Option.Some(inner) => inner,
    Option.None => 0,
}
}

fn main() -> Int {
let current: Option[Int] = Option.Some(7)
let missing: Option[Int] = Option.None
return choose(current) + choose(missing)
}
"#,
    );

    assert!(rendered.contains("define i64 @ql_1_choose("));
    assert!(rendered.contains("call i64 @ql_1_choose"));
    assert!(!rendered.contains("task-handle alias analysis"));
}

#[test]
fn emits_short_circuit_bool_expression_lowering() {
    let rendered = emit_with_mode(
        r#"
fn left_true() -> Bool {
return true
}

fn left_false() -> Bool {
return false
}

fn right_true() -> Bool {
return true
}

fn right_false() -> Bool {
return false
}

fn main() -> Int {
let both = left_false() && right_true()
let either = left_true() || right_false()

if both {
    return 1
}

if either && !both {
    return 3
}

return 4
}
"#,
        CodegenMode::Program,
    );

    assert!(rendered.contains("call i1 @ql_1_left_false()"));
    assert!(rendered.contains("call i1 @ql_2_right_true()"));
    assert!(rendered.contains("call i1 @ql_0_left_true()"));
    assert!(rendered.contains("call i1 @ql_3_right_false()"));
    assert!(rendered.contains("store i1 false"));
    assert!(rendered.contains("store i1 true"));
    assert!(rendered.contains("br i1"));
    assert!(!rendered.contains(" and i1 "));
    assert!(!rendered.contains(" or i1 "));
}

#[test]
fn emits_logical_bool_guard_match_lowering() {
    let rendered = emit_with_mode(
        r#"
fn main() -> Int {
let flag = true
let enabled = true
let blocked = false
return match flag {
    true if enabled && !blocked => 10,
    true if blocked || !enabled => 20,
    true => 30,
    false => 0,
}
}
"#,
        CodegenMode::Program,
    );

    assert!(rendered.contains("bb0_match_guard0:"));
    assert!(rendered.contains("bb0_match_guard1:"));
    assert!(rendered.contains(" and i1 "));
    assert!(rendered.contains(" or i1 "));
    assert!(!rendered.contains("does not support `match` lowering yet"));
}

#[test]
fn emits_match_guard_binding_operand_lowering() {
    let rendered = emit_with_mode(
        r#"
fn choose_flag(flag: Bool, enabled: Bool) -> Int {
return match flag {
    state if state && enabled => 10,
    true => 20,
    false => 0,
}
}

fn choose_value(value: Int, limit: Int) -> Int {
return match value {
    current if current > limit => 10,
    _ => 0,
}
}

fn main() -> Int {
return choose_flag(true, true) + choose_value(3, 1)
}
"#,
        CodegenMode::Program,
    );

    assert!(rendered.contains(" and i1 "));
    assert!(rendered.contains("icmp sgt i64"));
    assert!(!rendered.contains("does not support `match` lowering yet"));
}

#[test]
fn emits_match_guard_binding_index_operand_lowering() {
    let rendered = emit_with_mode(
        r#"
fn main() -> Int {
let values = [1, 3, 5]
let value = 1
return match value {
    current if values[current] < values[2] => 10,
    _ => 0,
}
}
"#,
        CodegenMode::Program,
    );

    assert!(rendered.contains("bb0_match_guard0:"));
    assert!(rendered.contains("%l6_current"));
    assert!(rendered.contains("getelementptr inbounds [3 x i64], ptr %l2_values, i64 0, i64 %"));
    assert!(rendered.contains("icmp slt i64"));
    assert!(!rendered.contains("does not support `match` lowering yet"));
}

#[test]
fn emits_match_guard_binding_projection_root_lowering() {
    let rendered = emit_with_mode(
        r#"
struct Slot {
ready: Bool,
value: Int,
}

struct State {
slot: Slot,
}

fn main() -> Int {
let state = State { slot: Slot { ready: true, value: 10 } }
let pair = (10, 2)
let values = [1, 7, 13]
let left = match state {
    current if current.slot.ready => 10,
    _ => 0,
}
let middle = match pair {
    current if current[1] == 2 => 12,
    _ => 0,
}
let right = match values {
    current if current[0] == 1 => 20,
    _ => 0,
}
return left + middle + right
}
"#,
        CodegenMode::Program,
    );

    assert!(rendered.matches("_match_guard0").count() >= 3);
    assert!(rendered.contains("load i1"));
    assert!(rendered.contains("getelementptr inbounds { { i1, i64 } }"));
    assert!(rendered.contains("getelementptr inbounds { i64, i64 }"));
    assert!(rendered.contains("getelementptr inbounds [3 x i64]"));
    assert!(rendered.contains("icmp eq i64"));
    assert!(!rendered.contains("does not support `match` lowering yet"));
}

#[test]
fn emits_match_binding_catch_all_lowering_for_aggregate_scrutinees() {
    let rendered = emit_with_mode(
        r#"
struct Slot {
ready: Bool,
value: Int,
}

struct State {
slot: Slot,
}

fn pick_state(state: State) -> Int {
return match state {
    current => current.slot.value,
}
}

fn pick_pair(pair: (Int, Int)) -> Int {
return match pair {
    current => current[0] + current[1],
}
}

fn pick_values(values: [Int; 3]) -> Int {
return match values {
    current => current[0] + current[2],
}
}

fn main() -> Int {
return pick_state(State {
    slot: Slot { ready: true, value: 10 },
}) + pick_pair((10, 2)) + pick_values([1, 7, 19])
}
"#,
        CodegenMode::Program,
    );

    assert!(rendered.matches("define i64 @ql_").count() >= 4);
    assert!(rendered.contains("getelementptr inbounds { { i1, i64 } }"));
    assert!(rendered.contains("getelementptr inbounds { i64, i64 }"));
    assert!(rendered.contains("getelementptr inbounds [3 x i64]"));
    assert!(!rendered.contains("does not support `match` lowering yet"));
}

#[test]
fn emits_match_destructuring_catch_all_lowering_for_aggregate_scrutinees() {
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
ready: Bool,
value: Int,
}

struct State {
slot: Slot,
}

extern "c" fn sink(value: Int)

fn pair_value() -> (Int, Int) {
return (3, 4)
}

async fn load_state(value: Int) -> State {
return State { slot: Slot { ready: true, value: value } }
}

async fn load_pair(value: Int) -> (Int, Int) {
return (value, value + 1)
}

const LOAD_STATE: (Int) -> Task[State] = load_state
const LOAD_PAIR: (Int) -> Task[(Int, Int)] = load_pair

async fn main() -> Int {
let branch = true
let direct = match pair_value() {
    (left, right) if left < right => left + right,
    _ => 0,
}
match await (if branch { pair_alias } else { pair_const_alias })(20) {
    (left, right) if left < right => sink(left + right),
    _ => sink(0),
}
match await (match branch { true => state_const_alias, false => state_alias })(13) {
    State { slot: Slot { value } } if value == 13 => sink(value),
    _ => sink(0),
}
return direct
}
"#,
        CodegenMode::Program,
        &runtime_hooks,
    );

    assert!(rendered.matches("extractvalue { i64, i64 }").count() >= 2);
    assert!(rendered.contains("extractvalue { { i1, i64 } }"));
    assert!(rendered.contains("icmp slt i64"));
    assert!(rendered.contains("icmp eq i64"));
    assert!(!rendered.contains("does not support `match` lowering yet"));
}

#[test]
fn emits_match_guard_runtime_index_expr_lowering() {
    let rendered = emit_with_mode(
        r#"
fn main() -> Int {
let values = [1, 3, 5]
let index = 0
let value = 0
return match value {
    current if values[index + 1] == values[current + 1] => 10,
    _ => 0,
}
}
"#,
        CodegenMode::Program,
    );

    assert!(rendered.contains("bb0_match_guard0:"));
    assert!(rendered.matches("add i64").count() >= 2);
    assert!(rendered.contains("getelementptr inbounds [3 x i64], ptr %l2_values, i64 0, i64 %"));
    assert!(rendered.contains("icmp eq i64"));
    assert!(!rendered.contains("does not support `match` lowering yet"));
}

#[test]
fn emits_match_guard_runtime_index_expr_lowering_for_item_aggregate_roots() {
    let rendered = emit_with_mode(
        r#"
use LIMITS as INPUT

const VALUES: [Int; 3] = [1, 3, 5]
static LIMITS: [Int; 3] = [2, 4, 6]

struct State {
offset: Int,
}

fn main() -> Int {
let index = 0
let state = State { offset: 1 }
let first = match 0 {
    0 if VALUES[index + 1] == 3 => 10,
    _ => 0,
}
let second = match 0 {
    0 if INPUT[state.offset] == 4 => 12,
    _ => 0,
}
let third = match 0 {
    0 if LIMITS[index + state.offset + 1] == 6 => 20,
    _ => 0,
}
return first + second + third
}
"#,
        CodegenMode::Program,
    );

    assert!(rendered.matches("_match_guard0").count() >= 3);
    assert!(rendered.matches("insertvalue [3 x i64]").count() >= 9);
    assert!(rendered.matches("getelementptr inbounds [3 x i64]").count() >= 3);
    assert!(rendered.matches("add i64").count() >= 2);
    assert!(rendered.contains("icmp eq i64"));
    assert!(!rendered.contains("does not support `match` lowering yet"));
}

#[test]
fn emits_const_path_match_lowering() {
    let rendered = emit_with_mode(
        r#"
use ENABLE as ON
use LIMIT as THRESHOLD

const ENABLE: Bool = true
const LIMIT: Int = 2

fn choose_flag(flag: Bool) -> Int {
return match flag {
    ON => 10,
    false => 0,
}
}

fn choose_value(value: Int) -> Int {
return match value {
    THRESHOLD => 20,
    _ => 0,
}
}

fn main() -> Int {
return choose_flag(true) + choose_value(2)
}
"#,
        CodegenMode::Program,
    );

    assert!(rendered.contains("br i1"));
    assert_eq!(rendered.matches("icmp eq i64").count(), 1);
    assert!(!rendered.contains("does not support `match` lowering yet"));
}

#[test]
fn emits_static_item_values_in_expressions() {
    let rendered = emit_with_mode(
        r#"
use LIMIT as THRESHOLD
use READY as ENABLED
use LIMITS as VALUES

static LIMIT: Int = 2
static READY: Bool = true
static LIMITS: [Int; 3] = [1, 3, 5]

fn main() -> Int {
let values = VALUES
let value = THRESHOLD + values[1]
if ENABLED {
    return value
}
return 0
}
"#,
        CodegenMode::Program,
    );

    assert!(rendered.contains("store [3 x i64]"));
    assert!(rendered.contains("getelementptr inbounds [3 x i64], ptr %l1_values, i64 0, i64 1"));
    assert!(rendered.contains("add i64 2, %"));
    assert!(rendered.contains("br i1 true"));
    assert!(!rendered.contains("does not support item values here"));
    assert!(!rendered.contains("does not support imported value lowering yet"));
}

#[test]
fn emits_static_path_and_guard_match_lowering() {
    let rendered = emit_with_mode(
        r#"
use ENABLE as ON
use LIMIT as THRESHOLD

static ENABLE: Bool = true
static LIMIT: Int = 2
static READY: Bool = LIMIT > 1

fn choose_flag(flag: Bool) -> Int {
return match flag {
    ON => 10,
    false => 0,
}
}

fn choose_value(value: Int) -> Int {
return match value {
    THRESHOLD if READY => 20,
    _ => 0,
}
}

fn main() -> Int {
return choose_flag(true) + choose_value(2)
}
"#,
        CodegenMode::Program,
    );

    assert!(rendered.contains("br i1"));
    assert_eq!(rendered.matches("icmp eq i64").count(), 1);
    assert!(!rendered.contains("does not support `match` lowering yet"));
}

#[test]
fn emits_computed_bool_const_path_and_guard_lowering() {
    let rendered = emit_with_mode(
        r#"
const BASE: Int = 1
const READY: Bool = BASE + 1 == 2
const SKIP: Bool = READY && BASE > 1

fn main() -> Int {
let flag = true
return match flag {
    READY if SKIP => 10,
    READY => 20,
    false => 0,
}
}
"#,
        CodegenMode::Program,
    );

    assert!(rendered.contains("br i1"));
    assert!(!rendered.contains("bb0_match_guard0:"));
    assert!(!rendered.contains("does not support `match` lowering yet"));
}

#[test]
fn emits_integer_match_lowering() {
    let rendered = emit_with_mode(
        r#"
fn main() -> Int {
let value = 2
return match value {
    1 => 10,
    2 => 20,
    _ => 0,
}
}
"#,
        CodegenMode::Program,
    );

    assert_eq!(rendered.matches("icmp eq i64").count(), 2);
    assert!(rendered.contains("bb0_match_dispatch1:"));
    assert!(!rendered.contains("does not support `match` lowering yet"));
}

#[test]
fn emits_integer_match_with_binding_catch_all_lowering() {
    let rendered = emit_with_mode(
        r#"
fn main() -> Int {
let value = 2
return match value {
    1 => 10,
    other => other,
}
}
"#,
        CodegenMode::Program,
    );

    assert_eq!(rendered.matches("icmp eq i64").count(), 1);
    assert!(rendered.contains("%l4_other = alloca i64"));
    assert!(rendered.contains("load i64, ptr %l4_other"));
    assert!(!rendered.contains("does not support `match` lowering yet"));
}

#[test]
fn emits_bool_match_with_literal_guard_lowering() {
    let rendered = emit_with_mode(
        r#"
fn main() -> Int {
let flag = false
return match flag {
    true if false => 1,
    true if true => 2,
    other if true => if other { 3 } else { 0 },
}
}
"#,
        CodegenMode::Program,
    );

    assert!(rendered.contains("%l4_other = alloca i1"));
    assert!(rendered.contains("load i1, ptr %l4_other"));
    assert!(!rendered.contains("does not support `match` lowering yet"));
}

#[test]
fn emits_integer_match_with_const_guard_lowering() {
    let rendered = emit_with_mode(
        r#"
const ENABLE: Bool = true
const DISABLE: Bool = false

fn main() -> Int {
let value = 2
return match value {
    1 if DISABLE => 10,
    2 if ENABLE => 20,
    other if ENABLE => other,
}
}
"#,
        CodegenMode::Program,
    );

    assert_eq!(rendered.matches("icmp eq i64").count(), 1);
    assert!(rendered.contains("%l4_other = alloca i64"));
    assert!(!rendered.contains("does not support `match` lowering yet"));
}

#[test]
fn emits_integer_guarded_match_lowering_without_catch_all_fallback() {
    let rendered = emit_with_mode(
        r#"
fn choose(value: Int, enabled: Bool) -> Int {
return match value {
    1 if enabled => 10,
    2 => 20,
}
}

fn main() -> Int {
return choose(1, true)
}
"#,
        CodegenMode::Program,
    );

    assert!(rendered.contains("bb0_match_guard0:"));
    assert!(rendered.contains("load i1, ptr %l2_enabled"));
    assert!(!rendered.contains("does not support `match` lowering yet"));
}

#[test]
fn emits_match_lowering_for_scrutinee_self_guard() {
    let rendered = emit_with_mode(
        r#"
fn choose(flag: Bool) -> Int {
return match flag {
    true if flag => 1,
    false => 0,
}
}

fn main() -> Int {
return choose(true)
}
"#,
        CodegenMode::Program,
    );

    assert!(rendered.contains("br i1"));
    assert!(!rendered.contains("does not support `match` lowering yet"));
}

#[test]
fn emits_match_lowering_for_scrutinee_bool_comparison_guard() {
    let rendered = emit_with_mode(
        r#"
use ENABLE as ON

const ENABLE: Bool = true

fn choose(flag: Bool) -> Int {
return match flag {
    true if flag == ON => 1,
    false => 0,
}
}

fn main() -> Int {
return choose(true)
}
"#,
        CodegenMode::Program,
    );

    assert!(rendered.contains("br i1"));
    assert!(!rendered.contains("bb0_match_guard0:"));
    assert!(!rendered.contains("does not support `match` lowering yet"));
}

#[test]
fn emits_guarded_match_lowering_without_true_fallback() {
    let rendered = emit_with_mode(
        r#"
fn main() -> Int {
let flag = true
let enabled = false
return match flag {
    true if enabled => 1,
    false => 0,
}
}
"#,
        CodegenMode::Program,
    );

    assert!(rendered.contains("bb0_match_guard0:"));
    assert!(rendered.contains("load i1, ptr %l2_enabled"));
    assert!(!rendered.contains("does not support `match` lowering yet"));
}

#[test]
fn emits_match_guard_direct_call_lowering() {
    let rendered = emit_with_mode(
        r#"
use shift as offset

fn enabled() -> Bool {
return true
}

fn shift(value: Int, delta: Int) -> Int {
return value + delta
}

fn main() -> Int {
let first = match true {
    true if enabled() => 10,
    false => 0,
}
let second = match 20 {
    current if offset(delta: 2, value: current) == 22 => 32,
    _ => 0,
}
return first + second
}
"#,
        CodegenMode::Program,
    );

    assert!(rendered.matches("_match_guard0").count() >= 2);
    assert!(rendered.matches("call i1 @ql_").count() >= 1);
    assert!(rendered.matches("call i64 @ql_").count() >= 1);
    assert!(rendered.contains("icmp eq i64"));
    assert!(!rendered.contains("does not support `match` lowering yet"));
}

#[test]
fn emits_match_guard_callable_alias_lowering() {
    let rendered = emit_with_mode(
        r#"
use READY as ready
use SHIFT as offset

fn enabled() -> Bool {
return true
}

fn shift(value: Int, delta: Int) -> Int {
return value + delta
}

const READY: () -> Bool = enabled
const SHIFT: (Int, Int) -> Int = shift

fn main() -> Int {
let first = match true {
    true if ready() => 10,
    false => 0,
}
let second = match 20 {
    current if offset(current, 2) == 22 => 32,
    _ => 0,
}
return first + second
}
"#,
        CodegenMode::Program,
    );

    assert!(rendered.matches("_match_guard0").count() >= 2);
    assert!(rendered.contains("call i1 %t"));
    assert!(rendered.contains("call i64 %t"));
    assert!(rendered.contains("icmp eq i64"));
    assert!(!rendered.contains("does not support `match` lowering yet"));
}

#[test]
fn emits_match_guard_call_projection_root_lowering() {
    let rendered = emit_with_mode(
        r#"
struct State {
value: Int,
}

fn pair(value: Int) -> (Int, Int) {
return (0, value)
}

fn state(value: Int) -> State {
return State { value: value }
}

fn values(seed: Int) -> [Int; 3] {
return [seed, seed + 1, seed + 2]
}

fn main() -> Int {
let first = match 22 {
    current if pair(current)[1] == 22 => 10,
    _ => 0,
}
let second = match 12 {
    current if state(current).value == 12 => 12,
    _ => 0,
}
let third = match 3 {
    current if values(current)[1] == 4 => 20,
    _ => 0,
}
return first + second + third
}
"#,
        CodegenMode::Program,
    );

    assert!(rendered.matches("_match_guard0").count() >= 3);
    assert!(rendered.matches("call { i64, i64 } @ql_").count() >= 1);
    assert!(rendered.matches("call { i64 } @ql_").count() >= 1);
    assert!(rendered.matches("call [3 x i64] @ql_").count() >= 1);
    assert!(!rendered.contains("does not support `match` lowering yet"));
}

#[test]
fn emits_match_guard_aggregate_call_arg_lowering() {
    let rendered = emit_with_mode(
        r#"
struct State {
ready: Bool,
}

fn enabled(state: State) -> Bool {
return state.ready
}

fn pair(value: Int) -> (Int, Int) {
return (0, value)
}

fn matches(pair: (Int, Int), expected: Int) -> Bool {
return pair[1] == expected
}

fn values(seed: Int) -> [Int; 3] {
return [seed, seed + 1, seed + 2]
}

fn contains(values: [Int; 3], expected: Int) -> Bool {
return values[1] == expected
}

fn main() -> Int {
let state = State { ready: true }
let first = match state {
    current if enabled(current) => 10,
    _ => 0,
}
let second = match 22 {
    current if matches(pair(current), 22) => 12,
    _ => 0,
}
let third = match 3 {
    current if contains(values(current), 4) => 20,
    _ => 0,
}
return first + second + third
}
"#,
        CodegenMode::Program,
    );

    assert!(rendered.matches("_match_guard0").count() >= 3);
    assert!(rendered.matches("call i1 @ql_").count() >= 3);
    assert!(rendered.contains("call i1 @ql_1_enabled({ i1 }"));
    assert!(rendered.contains("call i1 @ql_3_matches({ i64, i64 }"));
    assert!(rendered.contains("call i1 @ql_5_contains([3 x i64]"));
    assert!(!rendered.contains("does not support `match` lowering yet"));
}

#[test]
fn emits_match_guard_inline_aggregate_call_arg_lowering() {
    let rendered = emit_with_mode(
        r#"
struct State {
ready: Bool,
}

fn enabled(state: State) -> Bool {
return state.ready
}

fn matches(pair: (Int, Int), expected: Int) -> Bool {
return pair[1] == expected
}

fn contains(values: [Int; 3], expected: Int) -> Bool {
return values[1] == expected
}

fn main() -> Int {
let first = match true {
    true if enabled(State { ready: true }) => 10,
    false => 0,
}
let second = match 22 {
    current if matches((0, current), 22) => 12,
    _ => 0,
}
let third = match 3 {
    current if contains([current, current + 1, current + 2], 4) => 20,
    _ => 0,
}
return first + second + third
}
"#,
        CodegenMode::Program,
    );

    assert!(rendered.matches("_match_guard0").count() >= 3);
    assert!(rendered.matches("call i1 @ql_").count() >= 3);
    assert!(rendered.contains("insertvalue { i1 } undef, i1 true, 0"));
    assert!(rendered.contains("insertvalue { i64, i64 } undef, i64 0, 0"));
    assert!(rendered.contains("insertvalue [3 x i64] undef"));
    assert!(rendered.contains("add i64 %"));
    assert!(!rendered.contains("does not support `match` lowering yet"));
}

#[test]
fn emits_match_guard_inline_projection_root_lowering() {
    let rendered = emit_with_mode(
        r#"
struct State {
value: Int,
}

fn main() -> Int {
let value = 22
let first = match value {
    current if (0, current)[1] == 22 => 10,
    _ => 0,
}
let second = match value {
    current if State { value: current }.value == 22 => 12,
    _ => 0,
}
let third = match 3 {
    current if [current, current + 1, current + 2][1] == 4 => 20,
    _ => 0,
}
return first + second + third
}
"#,
        CodegenMode::Program,
    );

    assert!(rendered.matches("_match_guard0").count() >= 3);
    assert!(!rendered.contains("does not support `match` lowering yet"));
}

#[test]
fn emits_match_guard_item_backed_inline_combo_lowering() {
    let rendered = emit_with_mode(
        r#"
use LIMITS as INPUT
use check as enabled

static LIMITS: [Int; 3] = [3, 4, 5]

struct State {
ready: Bool,
value: Int,
}

static READY: State = State { ready: true, value: 22 }

fn check(state: State, extra: Bool) -> Bool {
return state.ready && extra
}

fn main() -> Int {
let state = State { ready: true, value: 7 }
let first = match true {
    true if enabled(extra: true, state: state) => 10,
    false => 0,
}
let second = match 22 {
    current if (INPUT[0], current)[1] == READY.value => 12,
    _ => 0,
}
let third = match 3 {
    current if [INPUT[0], current + 1, INPUT[2]][current - 2] == 4 => 20,
    _ => 0,
}
return first + second + third
}
"#,
        CodegenMode::Program,
    );

    assert!(rendered.matches("_match_guard0").count() >= 3);
    assert!(rendered.contains("call i1 @ql_"));
    assert!(rendered.contains("insertvalue [3 x i64] undef, i64 3, 0"));
    assert!(rendered.contains("insertvalue { i1, i64 } undef, i1 true, 0"));
    assert!(rendered.contains("getelementptr inbounds [3 x i64], ptr"));
    assert!(rendered.contains("sub i64 %"));
    assert!(!rendered.contains("does not support `match` lowering yet"));
}

#[test]
fn emits_match_guard_call_backed_combo_lowering() {
    let rendered = emit_with_mode(
        r#"
use values as items
use offset as slot

struct State {
ready: Bool,
}

fn ready(flag: Bool) -> Bool {
return flag
}

fn enabled(state: State, extra: Bool) -> Bool {
return state.ready && extra
}

fn seed(value: Int) -> Int {
return value
}

fn matches(pair: (Int, Int), expected: Int) -> Bool {
return pair[1] == expected
}

fn values(seed: Int) -> [Int; 3] {
return [seed, seed + 1, seed + 2]
}

fn offset(value: Int) -> Int {
return value - 2
}

fn main() -> Int {
let first = match true {
    true if enabled(extra: ready(true), state: State { ready: ready(true) }) => 10,
    false => 0,
}
let second = match 22 {
    current if matches((seed(0), current), 22) => 12,
    _ => 0,
}
let third = match 3 {
    current if items(current)[slot(current)] == 4 => 20,
    _ => 0,
}
return first + second + third
}
"#,
        CodegenMode::Program,
    );

    assert!(rendered.matches("_match_guard0").count() >= 3);
    assert!(rendered.contains("call i1 @ql_"));
    assert!(rendered.contains("call i64 @ql_"));
    assert!(rendered.contains("call [3 x i64] @ql_"));
    assert!(rendered.contains("insertvalue { i1 } undef, i1 %"));
    assert!(rendered.contains("insertvalue { i64, i64 }"));
    assert!(rendered.contains("getelementptr inbounds [3 x i64], ptr"));
    assert!(rendered.contains("sub i64 %"));
    assert!(!rendered.contains("does not support `match` lowering yet"));
}

#[test]
fn emits_match_guard_call_root_nested_runtime_projection_lowering() {
    let rendered = emit_with_mode(
        r#"
use bundle as pack
use matches as check

struct Bundle {
values: [Int; 3],
}

fn bundle(seed: Int) -> Bundle {
return Bundle { values: [seed, seed + 1, seed + 2] }
}

fn offset(value: Int) -> Int {
return value - 2
}

fn ready(value: Int) -> Bool {
return value == 4
}

fn matches(value: Int, expected: Int) -> Bool {
return value == expected
}

fn main() -> Int {
let first = match 3 {
    current if pack(current).values[offset(current)] == 4 => 10,
    _ => 0,
}
let second = match 3 {
    current if ready(pack(current).values[offset(current)]) => 12,
    _ => 0,
}
let third = match 3 {
    current if check(expected: 4, value: pack(current).values[offset(current)]) => 20,
    _ => 0,
}
return first + second + third
}
"#,
        CodegenMode::Program,
    );

    assert!(rendered.matches("_match_guard0").count() >= 3);
    assert!(rendered.contains("call { [3 x i64] } @ql_"));
    assert!(rendered.contains("call i64 @ql_"));
    assert!(rendered.contains("call i1 @ql_"));
    assert!(rendered.contains("getelementptr inbounds { [3 x i64] }, ptr"));
    assert!(rendered.contains("getelementptr inbounds [3 x i64], ptr"));
    assert!(rendered.contains("sub i64 %"));
    assert!(!rendered.contains("does not support `match` lowering yet"));
}

#[test]
fn emits_match_guard_nested_call_root_inline_combo_lowering() {
    let rendered = emit_with_mode(
        r#"
use bundle as pack
use offset as slot
use matches as check

fn bundle(seed: Int) -> [Int; 3] {
return [seed, seed + 1, seed + 2]
}

fn offset(value: Int) -> Int {
return value - 2
}

fn matches(value: Int, expected: Int) -> Bool {
return value == expected
}

fn pair(left: Int, right: Int) -> (Int, Int) {
return (left, right)
}

fn contains(values: [Int; 3], expected: Int) -> Bool {
return values[0] == expected
}

fn main() -> Int {
let first = match 3 {
    current if [pack(current)[slot(current)], current + 1, 6][0] == 4 => 10,
    _ => 0,
}
let second = match 22 {
    current if contains([pack(3)[slot(3)], current, 9], 4) => 12,
    _ => 0,
}
let third = match 3 {
    current if check(expected: 4, value: pair(left: pack(current)[slot(current)], right: 8)[0]) => 20,
    _ => 0,
}
return first + second + third
}
"#,
        CodegenMode::Program,
    );

    assert!(rendered.matches("_match_guard0").count() >= 3);
    assert!(rendered.contains("call [3 x i64] @ql_"));
    assert!(rendered.contains("call i64 @ql_"));
    assert!(rendered.contains("call i1 @ql_"));
    assert!(rendered.contains("insertvalue [3 x i64]"));
    assert!(rendered.contains("insertvalue { i64, i64 }"));
    assert!(rendered.contains("getelementptr inbounds [3 x i64], ptr"));
    assert!(rendered.contains("getelementptr inbounds { i64, i64 }, ptr"));
    assert!(rendered.contains("sub i64 %"));
    assert!(!rendered.contains("does not support `match` lowering yet"));
}

#[test]
fn emits_match_guard_item_backed_nested_call_root_combo_lowering() {
    let rendered = emit_with_mode(
        r#"
use LIMITS as INPUT
use matches as check

static LIMITS: [Int; 3] = [4, 8, 9]

struct State {
ready: Bool,
}

fn state(flag: Bool) -> State {
return State { ready: flag }
}

fn bundle(seed: Int) -> [Int; 3] {
return [seed, seed + 1, seed + 2]
}

fn offset(value: Int) -> Int {
return value - 2
}

fn enabled(state: State, extra: Bool) -> Bool {
return state.ready && extra
}

fn matches(value: Int, expected: Int) -> Bool {
return value == expected
}

fn main() -> Int {
let first = match true {
    true if enabled(extra: INPUT[0] == bundle(3)[offset(3)], state: state(bundle(3)[offset(3)] == 4)) => 10,
    false => 0,
}
let second = match 3 {
    current if [bundle(current)[offset(current)], INPUT[1], INPUT[2]][0] == INPUT[0] => 12,
    _ => 0,
}
let third = match 3 {
    current if check(expected: INPUT[0], value: [bundle(current)[offset(current)], 8, 9][0]) => 20,
    _ => 0,
}
return first + second + third
}
"#,
        CodegenMode::Program,
    );

    assert!(rendered.matches("_match_guard0").count() >= 3);
    assert!(rendered.contains("call [3 x i64] @ql_"));
    assert!(rendered.contains("call i1 @ql_"));
    assert!(rendered.contains("insertvalue [3 x i64]"));
    assert!(rendered.contains("getelementptr inbounds [3 x i64], ptr"));
    assert!(rendered.contains("sub i64 %"));
    assert!(rendered.contains("icmp eq i64"));
    assert!(!rendered.contains("does not support `match` lowering yet"));
}

#[test]
fn emits_match_guard_call_backed_nested_call_root_combo_lowering() {
    let rendered = emit_with_mode(
        r#"
use bundle as pack
use offset as slot
use matches as check
use ready as flag

struct State {
ready: Bool,
}

fn state(flag: Bool) -> State {
return State { ready: flag }
}

fn bundle(seed: Int) -> [Int; 3] {
return [seed, seed + 1, seed + 2]
}

fn offset(value: Int) -> Int {
return value - 2
}

fn ready(flag: Bool) -> Bool {
return flag
}

fn seed(value: Int) -> Int {
return value
}

fn enabled(state: State, extra: Bool) -> Bool {
return state.ready && extra
}

fn matches(value: Int, expected: Int) -> Bool {
return value == expected
}

fn main() -> Int {
let first = match true {
    true if enabled(extra: flag(pack(3)[slot(3)] == 4), state: state(flag(pack(3)[slot(3)] == 4))) => 10,
    false => 0,
}
let second = match 3 {
    current if [pack(current)[slot(current)], seed(8), seed(9)][0] == seed(4) => 12,
    _ => 0,
}
let third = match 3 {
    current if check(expected: seed(4), value: [pack(current)[slot(current)], seed(8), 9][0]) => 20,
    _ => 0,
}
return first + second + third
}
"#,
        CodegenMode::Program,
    );

    assert!(rendered.matches("_match_guard0").count() >= 3);
    assert!(rendered.contains("call [3 x i64] @ql_"));
    assert!(rendered.contains("call i64 @ql_"));
    assert!(rendered.contains("call i1 @ql_"));
    assert!(rendered.contains("insertvalue [3 x i64]"));
    assert!(rendered.contains("getelementptr inbounds [3 x i64], ptr"));
    assert!(rendered.contains("sub i64 %"));
    assert!(rendered.contains("icmp eq i64"));
    assert!(!rendered.contains("does not support `match` lowering yet"));
}

#[test]
fn emits_match_guard_alias_backed_nested_call_root_combo_lowering() {
    let rendered = emit_with_mode(
        r#"
use bundle as pack
use offset as slot
use ready as flag
use enabled as allow
use state as make
use matches as check
use seed as literal

struct State {
ready: Bool,
}

fn state(flag: Bool) -> State {
return State { ready: flag }
}

fn bundle(seed: Int) -> [Int; 3] {
return [seed, seed + 1, seed + 2]
}

fn offset(value: Int) -> Int {
return value - 2
}

fn ready(flag: Bool) -> Bool {
return flag
}

fn enabled(state: State, extra: Bool) -> Bool {
return state.ready && extra
}

fn matches(value: Int, expected: Int) -> Bool {
return value == expected
}

fn seed(value: Int) -> Int {
return value
}

fn main() -> Int {
let first = match true {
    true if allow(extra: flag(pack(3)[slot(3)] == literal(4)), state: make(flag(pack(3)[slot(3)] == literal(4)))) => 10,
    false => 0,
}
let second = match 3 {
    current if [pack(current)[slot(current)], literal(8), literal(9)][0] == literal(4) => 12,
    _ => 0,
}
let third = match 3 {
    current if check(expected: literal(4), value: [pack(current)[slot(current)], literal(8), 9][0]) => 20,
    _ => 0,
}
return first + second + third
}
"#,
        CodegenMode::Program,
    );

    assert!(rendered.matches("_match_guard0").count() >= 3);
    assert!(rendered.contains("call [3 x i64] @ql_"));
    assert!(rendered.contains("call i64 @ql_"));
    assert!(rendered.contains("call i1 @ql_"));
    assert!(rendered.contains("insertvalue [3 x i64]"));
    assert!(rendered.contains("getelementptr inbounds [3 x i64], ptr"));
    assert!(rendered.contains("sub i64 %"));
    assert!(rendered.contains("icmp eq i64"));
    assert!(!rendered.contains("does not support `match` lowering yet"));
}

#[test]
fn emits_match_guard_binding_backed_nested_call_root_combo_lowering() {
    let rendered = emit_with_mode(
        r#"
struct State {
ready: Bool,
value: Int,
}

fn state(flag: Bool, value: Int) -> State {
return State { ready: flag, value: value }
}

fn bundle(seed: Int) -> [Int; 3] {
return [seed, seed + 1, seed + 2]
}

fn offset(value: Int) -> Int {
return value - 2
}

fn enabled(state: State, extra: Bool) -> Bool {
return state.ready && extra
}

fn matches(value: Int, expected: Int) -> Bool {
return value == expected
}

fn main() -> Int {
let first = match state(flag: true, value: 3) {
    current if enabled(extra: bundle(current.value)[offset(current.value)] == 4, state: current) => 10,
    _ => 0,
}
let second = match state(flag: true, value: 3) {
    current if [bundle(current.value)[offset(current.value)], current.value + 5, 9][0] == 4 => 12,
    _ => 0,
}
let third = match state(flag: true, value: 3) {
    current if matches(expected: 4, value: [bundle(current.value)[offset(current.value)], current.value, 9][0]) => 20,
    _ => 0,
}
return first + second + third
}
"#,
        CodegenMode::Program,
    );

    assert!(rendered.matches("_match_guard0").count() >= 3);
    assert!(rendered.contains("call { i1, i64 } @ql_"));
    assert!(rendered.contains("call [3 x i64] @ql_"));
    assert!(rendered.contains("call i1 @ql_"));
    assert!(rendered.contains("insertvalue [3 x i64]"));
    assert!(rendered.contains("getelementptr inbounds { i1, i64 }, ptr"));
    assert!(rendered.contains("getelementptr inbounds [3 x i64], ptr"));
    assert!(rendered.contains("sub i64 %"));
    assert!(rendered.contains("icmp eq i64"));
    assert!(!rendered.contains("does not support `match` lowering yet"));
}

#[test]
fn emits_match_guard_projection_backed_nested_call_root_combo_lowering() {
    let rendered = emit_with_mode(
        r#"
struct Slot {
value: Int,
}

struct Config {
slot: Slot,
}

struct State {
ready: Bool,
}

fn state(flag: Bool) -> State {
return State { ready: flag }
}

fn bundle(seed: Int) -> [Int; 3] {
return [seed, seed + 1, seed + 2]
}

fn offset(value: Int) -> Int {
return value - 2
}

fn enabled(state: State, extra: Bool) -> Bool {
return state.ready && extra
}

fn matches(value: Int, expected: Int) -> Bool {
return value == expected
}

fn main() -> Int {
let config = Config {
    slot: Slot { value: 3 },
}
let first = match true {
    true if enabled(extra: bundle(config.slot.value)[offset(config.slot.value)] == 4, state: state(bundle(config.slot.value)[offset(config.slot.value)] == 4)) => 10,
    false => 0,
}
let second = match 3 {
    current if [bundle(config.slot.value)[offset(config.slot.value)], current + 5, 9][0] == 4 => 12,
    _ => 0,
}
let third = match 3 {
    current if matches(expected: 4, value: [bundle(config.slot.value)[offset(config.slot.value)], current, 9][0]) => 20,
    _ => 0,
}
return first + second + third
}
"#,
        CodegenMode::Program,
    );

    assert!(rendered.matches("_match_guard0").count() >= 3);
    assert!(rendered.contains("call [3 x i64] @ql_"));
    assert!(rendered.contains("call i1 @ql_"));
    assert!(rendered.contains("call { i1 } @ql_"));
    assert!(rendered.contains("insertvalue [3 x i64]"));
    assert!(rendered.contains("getelementptr inbounds { { i64 } }, ptr"));
    assert!(rendered.contains("getelementptr inbounds { i64 }, ptr"));
    assert!(rendered.contains("getelementptr inbounds [3 x i64], ptr"));
    assert!(rendered.contains("sub i64 %"));
    assert!(rendered.contains("icmp eq i64"));
    assert!(!rendered.contains("does not support `match` lowering yet"));
}

#[test]
fn emits_negative_int_const_path_and_guard_lowering() {
    let rendered = emit_with_mode(
        r#"
use LIMIT as THRESHOLD

const LIMIT: Int = -1

fn main() -> Int {
let value = 0
return match value {
    THRESHOLD => 10,
    0 if value > -2 => 20,
    _ => 0,
}
}
"#,
        CodegenMode::Program,
    );

    assert!(rendered.contains("icmp eq i64 %t1, -1"));
    assert!(rendered.contains("sub i64 0, 2"));
    assert!(rendered.contains("icmp sgt i64"));
    assert!(!rendered.contains("does not support `match` lowering yet"));
}

#[test]
fn emits_const_arithmetic_path_and_guard_operand_lowering() {
    let rendered = emit_with_mode(
        r#"
const BASE: Int = 1
const LIMIT: Int = BASE + 1

fn main() -> Int {
let value = 2
return match value {
    LIMIT if value + BASE == LIMIT + 1 => 10,
    _ => 0,
}
}
"#,
        CodegenMode::Program,
    );

    assert!(rendered.contains("icmp eq i64 %t1, 2"));
    assert!(rendered.matches("add i64").count() >= 2);
    assert!(rendered.matches("icmp eq i64").count() >= 2);
    assert!(!rendered.contains("does not support `match` lowering yet"));
}

#[test]
fn emits_projected_guard_with_folded_const_arithmetic_indices() {
    let rendered = emit_with_mode(
        r#"
const BASE: Int = 1

struct State {
pair: (Int, Int, Int),
values: [Int; 3],
}

fn main() -> Int {
let value = 3
let state = State {
    pair: (1, 2, 4),
    values: [1, 2, 4],
}
return match value {
    3 if state.pair[BASE + 1] == state.values[BASE + 1] => 30,
    _ => 0,
}
}
"#,
        CodegenMode::Program,
    );

    assert!(rendered.contains("bb0_match_guard0:"));
    assert!(rendered.contains("icmp eq i64"));
    assert!(!rendered.contains("does not support `match` lowering yet"));
}

#[test]
fn emits_static_projected_integer_guard_lowering() {
    let rendered = emit_with_mode(
        r#"
use STATE as CURRENT

struct Slot {
value: Int,
}

struct State {
slot: Slot,
pair: (Int, Int),
limits: [Int; 2],
}

static STATE: State = State {
slot: Slot { value: 2 },
pair: (0, 2),
limits: [1, 4],
}

fn main() -> Int {
let value = 3
return match value {
    3 if CURRENT.pair[1] == CURRENT.slot.value => 30,
    3 if CURRENT.limits[0] < CURRENT.slot.value => 31,
    _ => 0,
}
}
"#,
        CodegenMode::Program,
    );

    assert!(!rendered.contains("bb0_match_guard0:"));
    assert!(!rendered.contains("bb0_match_guard1:"));
    assert_eq!(rendered.matches("icmp eq i64").count(), 2);
    assert!(!rendered.contains("getelementptr inbounds { { i64 }, { i64, i64 }, [2 x i64] }"));
    assert!(!rendered.contains("does not support `match` lowering yet"));
}
