use STATE as CURRENT

struct Slot {
    ready: Bool,
}

struct State {
    slot: Slot,
    flags: [Bool; 2],
}

const STATE: State = State {
    slot: Slot { ready: false },
    flags: [true, false],
}

fn main() -> Int {
    let value = 3
    let state = State {
        slot: Slot { ready: false },
        flags: [true, false],
    }
    let open = !state.slot.ready
    return match value {
        1 if !false => 10,
        2 if !CURRENT.flags[1] => 20,
        3 if !(open == CURRENT.slot.ready) => 30,
        _ => 0,
    }
}
