use pack_values as pack
use pick_slot as slot
use truthy as flag
use scalar_matches as equal

struct FlagState {
    ready: Bool,
}

async fn fetch_flag(value: Bool) -> Bool {
    return value
}

async fn fetch_value(value: Int) -> Int {
    return value
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

fn scalar_matches(value: Int, expected: Int) -> Bool {
    return value == expected
}

async fn main() -> Int {
    let first = await fetch_flag(value: true)
    let from_bool = match first {
        true if enabled(extra: flag(pack(3)[slot(3)] == 4), state: flag_state(flag(pack(3)[slot(3)] == 4))) => 10,
        false => 0,
    }

    let second = await fetch_value(value: 3)
    let from_inline = match second {
        current if [pack(current)[slot(current)], seed(8), seed(9)][0] == seed(4) => 12,
        _ => 0,
    }

    let third = await fetch_value(value: 3)
    let from_guard_call = match third {
        current if equal(expected: seed(4), value: [pack(current)[slot(current)], seed(8), 9][0]) => 20,
        _ => 0,
    }

    return from_bool + from_inline + from_guard_call
}
