struct Slot {
    ready: Bool,
    value: Int,
}

struct State {
    slot: Slot,
}

fn from_struct(state: State) -> Int {
    return match state {
        current if current.slot.ready => 10,
        _ => 0,
    }
}

fn from_tuple(pair: (Int, Int)) -> Int {
    return match pair {
        current if current[1] == 2 => 12,
        _ => 0,
    }
}

fn from_array(values: [Int; 3]) -> Int {
    return match values {
        current if current[0] == 1 => 20,
        _ => 0,
    }
}

fn main() -> Int {
    return from_struct(State {
        slot: Slot { ready: true, value: 10 },
    }) + from_tuple((10, 2)) + from_array([1, 7, 13])
}
