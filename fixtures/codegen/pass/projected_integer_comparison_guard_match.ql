struct Slot {
    value: Int,
}

struct State {
    slot: Slot,
    pair: (Int, Int),
    values: [Int; 2],
}

fn main() -> Int {
    let value = 3
    let state = State {
        slot: Slot { value: 2 },
        pair: (0, 1),
        values: [1, 4],
    }
    return match value {
        3 if state.pair[1] == state.slot.value => 30,
        3 if state.values[0] < state.slot.value => 31,
        _ => 0,
    }
}
