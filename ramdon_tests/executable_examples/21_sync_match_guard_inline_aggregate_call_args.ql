struct State {
    ready: Bool,
}

fn enabled(state: State) -> Bool {
    return state.ready
}

fn matches(pair: (Int, Int), expected: Int) -> Bool {
    return pair[1] == expected
}

fn contains(values: [Int; 3], expected: Int) -> Bool {
    return values[1] == expected
}

fn main() -> Int {
    let first = match true {
        true if enabled(State { ready: true }) => 10,
        false => 0,
    }
    let second = match 22 {
        current if matches((0, current), 22) => 12,
        _ => 0,
    }
    let third = match 3 {
        current if contains([current, current + 1, current + 2], 4) => 20,
        _ => 0,
    }
    return first + second + third
}
