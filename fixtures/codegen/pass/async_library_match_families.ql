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

    return from_scalar + from_aggregate + from_pair_projection + from_pair_call_root
}
