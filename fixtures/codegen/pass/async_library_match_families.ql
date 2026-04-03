struct Slot {
    ready: Bool,
    value: Int,
}

struct State {
    slot: Slot,
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

fn offset(delta: Int, value: Int) -> Int {
    return value + delta
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

    return from_scalar + from_aggregate
}
