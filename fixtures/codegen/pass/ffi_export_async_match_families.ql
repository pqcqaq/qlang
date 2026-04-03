use fetch_value as load_scalar
use load_pair_state as load_pairs
use offset as shift
use matches as check
use pair as make_pair

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
}

extern "c" pub fn q_export() -> Int {
    return 1
}
