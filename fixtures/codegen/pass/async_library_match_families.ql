use fetch_value as load_scalar
use load_pair_state as load_pairs
use LIMITS as INPUT
use offset as shift
use matches as check
use binding_enabled as item_enabled
use pair as make_pair
use pack_values as pack
use pack_values as items
use pick_slot as slot
use truthy as flag
use scalar_matches as equal
use enabled as allow
use flag_state as make
use seed as literal

static LIMITS: [Int; 3] = [4, 8, 9]
static READY: BindingState = BindingState {
    ready: true,
    value: 22,
}

struct Slot {
    ready: Bool,
    value: Int,
}

struct State {
    slot: Slot,
}

struct Pair {
    left: Int,
    right: Int,
}

struct PairState {
    values: Pair,
}

struct Bundle {
    values: [Int; 3],
}

struct FlagState {
    ready: Bool,
}

struct BindingState {
    ready: Bool,
    value: Int,
}

struct IndexSlot {
    value: Int,
}

struct Config {
    slot: IndexSlot,
}

struct InlineReady {
    ready: Bool,
}

struct InlineValue {
    value: Int,
}

async fn fetch_flag(value: Bool) -> Bool {
    return value
}

async fn fetch_value(value: Int) -> Int {
    return value
}

async fn load_state(value: Int) -> State {
    return State {
        slot: Slot {
            ready: true,
            value: value,
        },
    }
}

async fn load_pair_state(value: Int) -> PairState {
    return PairState {
        values: Pair {
            left: value,
            right: value + 2,
        },
    }
}

fn offset(delta: Int, value: Int) -> Int {
    return value + delta
}

fn pair(value: Int) -> Pair {
    return Pair {
        left: value,
        right: value + 2,
    }
}

fn matches(expected: Int, value: Pair) -> Bool {
    return value.right == expected
}

fn tuple_matches(pair: (Int, Int), expected: Int) -> Bool {
    return pair[1] == expected
}

fn inline_enabled(state: InlineReady) -> Bool {
    return state.ready
}

fn contains(values: [Int; 3], expected: Int) -> Bool {
    return values[1] == expected
}

fn bundle(seed: Int) -> Bundle {
    return Bundle {
        values: [seed, seed + 1, seed + 2],
    }
}

fn slot(value: Int) -> Int {
    return value - 2
}

fn ready(value: Int) -> Bool {
    return value == 4
}

fn scalar_matches(value: Int, expected: Int) -> Bool {
    return value == expected
}

fn flag_state(flag: Bool) -> FlagState {
    return FlagState { ready: flag }
}

fn pack_values(seed: Int) -> [Int; 3] {
    return [seed, seed + 1, seed + 2]
}

fn pick_slot(value: Int) -> Int {
    return value - 2
}

fn truthy(flag: Bool) -> Bool {
    return flag
}

fn seed(value: Int) -> Int {
    return value
}

fn enabled(state: FlagState, extra: Bool) -> Bool {
    return state.ready && extra
}

async fn load_binding_state(flag: Bool, value: Int) -> BindingState {
    return BindingState { ready: flag, value: value }
}

fn binding_enabled(state: BindingState, extra: Bool) -> Bool {
    return state.ready && extra
}

async fn helper() -> Int {
    let first = await fetch_value(value: 20)
    let from_scalar = match first {
        current if offset(delta: 2, value: current) == 22 => 20,
        _ => 0,
    }

    let second = await load_state(value: 22)
    let from_aggregate = match second {
        current if current.slot.ready => current.slot.value,
        _ => 0,
    }

    let third = await load_pair_state(value: 20)
    let from_pair_projection = match third {
        current if matches(expected: 22, value: current.values) => 20,
        _ => 0,
    }

    let fourth = await load_pair_state(value: 20)
    let from_pair_call_root = match fourth {
        current if matches(expected: 22, value: pair(value: current.values.left)) => 22,
        _ => 0,
    }

    let fifth = await load_scalar(value: 20)
    let from_alias_scalar = match fifth {
        current if shift(delta: 2, value: current) == 22 => 20,
        _ => 0,
    }

    let sixth = await load_pairs(value: 20)
    let from_alias_projection = match sixth {
        current if check(expected: 22, value: current.values) => 20,
        _ => 0,
    }

    let seventh = await load_pairs(value: 20)
    let from_alias_call_root = match seventh {
        current if check(expected: 22, value: make_pair(value: current.values.left)) => 22,
        _ => 0,
    }

    let eighth = await load_scalar(value: 3)
    let from_nested_projection = match eighth {
        current if bundle(current).values[slot(current)] == 4 => 10,
        _ => 0,
    }

    let ninth = await load_scalar(value: 3)
    let from_nested_direct_call = match ninth {
        current if ready(bundle(current).values[slot(current)]) => 12,
        _ => 0,
    }

    let tenth = await load_scalar(value: 3)
    let from_nested_guard_call = match tenth {
        current if scalar_matches(value: bundle(current).values[slot(current)], expected: 4) => 20,
        _ => 0,
    }

    let eleventh = await fetch_flag(value: true)
    let from_call_backed_bool = match eleventh {
        true if enabled(extra: flag(pack(3)[slot(3)] == 4), state: flag_state(flag(pack(3)[slot(3)] == 4))) => 10,
        false => 0,
    }

    let twelfth = await load_scalar(value: 3)
    let from_call_backed_inline = match twelfth {
        current if [pack(current)[slot(current)], seed(8), seed(9)][0] == seed(4) => 12,
        _ => 0,
    }

    let thirteenth = await load_scalar(value: 3)
    let from_call_backed_guard_call = match thirteenth {
        current if equal(expected: seed(4), value: [pack(current)[slot(current)], seed(8), 9][0]) => 20,
        _ => 0,
    }

    let fourteenth = await fetch_flag(value: true)
    let from_alias_backed_bool = match fourteenth {
        true if allow(extra: flag(pack(3)[slot(3)] == literal(4)), state: make(flag(pack(3)[slot(3)] == literal(4)))) => 10,
        false => 0,
    }

    let fifteenth = await load_scalar(value: 3)
    let from_alias_backed_inline = match fifteenth {
        current if [pack(current)[slot(current)], literal(8), literal(9)][0] == literal(4) => 12,
        _ => 0,
    }

    let sixteenth = await load_scalar(value: 3)
    let from_alias_backed_guard_call = match sixteenth {
        current if equal(expected: literal(4), value: [pack(current)[slot(current)], literal(8), 9][0]) => 20,
        _ => 0,
    }

    let seventeenth = await load_binding_state(flag: true, value: 3)
    let from_binding_backed_bool = match seventeenth {
        current if binding_enabled(extra: pack(current.value)[slot(current.value)] == 4, state: current) => 10,
        _ => 0,
    }

    let eighteenth = await load_binding_state(flag: true, value: 3)
    let from_binding_backed_inline = match eighteenth {
        current if [pack(current.value)[slot(current.value)], current.value + 5, 9][0] == 4 => 12,
        _ => 0,
    }

    let nineteenth = await load_binding_state(flag: true, value: 3)
    let from_binding_backed_guard_call = match nineteenth {
        current if equal(expected: 4, value: [pack(current.value)[slot(current.value)], current.value, 9][0]) => 20,
        _ => 0,
    }

    let config = Config {
        slot: IndexSlot { value: 3 },
    }
    let twentieth = await fetch_flag(value: true)
    let from_projection_backed_bool = match twentieth {
        true if enabled(extra: pack(config.slot.value)[slot(config.slot.value)] == 4, state: make(pack(config.slot.value)[slot(config.slot.value)] == 4)) => 10,
        false => 0,
    }

    let twenty_first = await load_scalar(value: 3)
    let from_projection_backed_inline = match twenty_first {
        current if [pack(config.slot.value)[slot(config.slot.value)], current + 5, 9][0] == 4 => 12,
        _ => 0,
    }

    let twenty_second = await load_scalar(value: 3)
    let from_projection_backed_guard_call = match twenty_second {
        current if equal(expected: 4, value: [pack(config.slot.value)[slot(config.slot.value)], current, 9][0]) => 20,
        _ => 0,
    }

    let twenty_third = await fetch_flag(value: true)
    let from_item_backed_bool = match twenty_third {
        true if enabled(extra: INPUT[0] == pack(3)[slot(3)], state: flag_state(pack(3)[slot(3)] == 4)) => 10,
        false => 0,
    }

    let twenty_fourth = await load_scalar(value: 3)
    let from_item_backed_inline = match twenty_fourth {
        current if [pack(current)[slot(current)], INPUT[1], INPUT[2]][0] == INPUT[0] => 12,
        _ => 0,
    }

    let twenty_fifth = await load_scalar(value: 3)
    let from_item_backed_guard_call = match twenty_fifth {
        current if equal(expected: INPUT[0], value: [pack(current)[slot(current)], 8, 9][0]) => 20,
        _ => 0,
    }

    let twenty_sixth = await fetch_flag(value: true)
    let from_item_backed_alias_guard = match twenty_sixth {
        true if item_enabled(extra: true, state: BindingState { ready: true, value: 7 }) => 10,
        false => 0,
    }

    let twenty_seventh = await load_scalar(value: 22)
    let from_item_backed_tuple_inline = match twenty_seventh {
        current if (INPUT[0], current)[1] == READY.value => 12,
        _ => 0,
    }

    let twenty_eighth = await load_scalar(value: 3)
    let from_item_backed_array_inline = match twenty_eighth {
        current if [INPUT[0], current + 1, INPUT[2]][current - 2] == 4 => 20,
        _ => 0,
    }

    let twenty_ninth = await fetch_flag(value: true)
    let from_call_backed_direct_guard = match twenty_ninth {
        true if enabled(extra: flag(true), state: FlagState { ready: flag(true) }) => 10,
        false => 0,
    }

    let thirtieth = await load_scalar(value: 22)
    let from_call_backed_tuple_inline = match thirtieth {
        current if tuple_matches((seed(0), current), 22) => 12,
        _ => 0,
    }

    let thirty_first = await load_scalar(value: 3)
    let from_call_backed_direct_root = match thirty_first {
        current if items(current)[slot(current)] == 4 => 20,
        _ => 0,
    }

    let thirty_second = await fetch_flag(value: true)
    let from_inline_struct_arg = match thirty_second {
        true if inline_enabled(InlineReady { ready: true }) => 10,
        false => 0,
    }

    let thirty_third = await load_scalar(value: 22)
    let from_inline_tuple_arg = match thirty_third {
        current if tuple_matches((0, current), 22) => 12,
        _ => 0,
    }

    let thirty_fourth = await load_scalar(value: 3)
    let from_inline_array_arg = match thirty_fourth {
        current if contains([current, current + 1, current + 2], 4) => 20,
        _ => 0,
    }

    let thirty_fifth = await load_scalar(value: 22)
    let from_inline_tuple_projection = match thirty_fifth {
        current if (0, current)[1] == 22 => 10,
        _ => 0,
    }

    let thirty_sixth = await load_scalar(value: 22)
    let from_inline_struct_projection = match thirty_sixth {
        current if InlineValue { value: current }.value == 22 => 12,
        _ => 0,
    }

    let thirty_seventh = await load_scalar(value: 3)
    let from_inline_array_projection = match thirty_seventh {
        current if [current, current + 1, current + 2][1] == 4 => 20,
        _ => 0,
    }

    return from_scalar
        + from_aggregate
        + from_pair_projection
        + from_pair_call_root
        + from_alias_scalar
        + from_alias_projection
        + from_alias_call_root
        + from_nested_projection
        + from_nested_direct_call
        + from_nested_guard_call
        + from_call_backed_bool
        + from_call_backed_inline
        + from_call_backed_guard_call
        + from_alias_backed_bool
        + from_alias_backed_inline
        + from_alias_backed_guard_call
        + from_binding_backed_bool
        + from_binding_backed_inline
        + from_binding_backed_guard_call
        + from_projection_backed_bool
        + from_projection_backed_inline
        + from_projection_backed_guard_call
        + from_item_backed_bool
        + from_item_backed_inline
        + from_item_backed_guard_call
        + from_item_backed_alias_guard
        + from_item_backed_tuple_inline
        + from_item_backed_array_inline
        + from_call_backed_direct_guard
        + from_call_backed_tuple_inline
        + from_call_backed_direct_root
        + from_inline_struct_arg
        + from_inline_tuple_arg
        + from_inline_array_arg
        + from_inline_tuple_projection
        + from_inline_struct_projection
        + from_inline_array_projection
}
