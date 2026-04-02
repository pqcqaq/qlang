use STATE as CURRENT

struct Slot {
    value: Int,
}

struct State {
    slot: Slot,
    pair: (Int, Int),
    limits: [Int; 2],
}

const STATE: State = State {
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
