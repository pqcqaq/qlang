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
