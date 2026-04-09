struct Slot {
    value: Int,
}

struct State {
    slot: Slot,
}

extern "c" fn sink(value: Int)

async fn worker(value: Int) -> State {
    return State {
        slot: Slot { value: value + 1 },
    }
}

fn wrap(state: State) -> State {
    return state
}

fn offset(value: Int) -> Int {
    return value - 11
}

fn matches(value: Int, expected: Int) -> Bool {
    return value == expected
}

async fn main() -> Int {
    let branch = true
    let which = 1
    let first = spawn worker(12)
    let second = spawn worker(14)
    let left = () => first
    let right = () => second

    defer if wrap(await (if branch { left } else { right })()).slot.value == 13 {
        sink(1);
    }

    defer match true {
        true if matches(
            value: [wrap(await (match which { 1 => right, _ => left })()).slot.value, 0][offset(11)],
            expected: 15,
        ) => sink(2),
        _ => sink(3),
    }

    defer match wrap(await (if branch { left } else { right })()).slot.value {
        13 => sink(4),
        _ => sink(5),
    }

    defer match [wrap(await (match which { 1 => right, _ => left })()).slot.value, 0][offset(11)] {
        15 => sink(6),
        _ => sink(7),
    }
    return 0
}
