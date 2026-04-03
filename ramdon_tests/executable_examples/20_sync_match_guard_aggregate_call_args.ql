struct State {
    ready: Bool,
}

fn enabled(state: State) -> Bool {
    return state.ready
}

fn pair(value: Int) -> (Int, Int) {
    return (0, value)
}

fn matches(pair: (Int, Int), expected: Int) -> Bool {
    return pair[1] == expected
}

fn values(seed: Int) -> [Int; 3] {
    return [seed, seed + 1, seed + 2]
}

fn contains(values: [Int; 3], expected: Int) -> Bool {
    return values[1] == expected
}

fn main() -> Int {
    let state = State { ready: true }
    let first = match state {
        current if enabled(current) => 10,
        _ => 0,
    }
    let second = match 22 {
        current if matches(pair(current), 22) => 12,
        _ => 0,
    }
    let third = match 3 {
        current if contains(values(current), 4) => 20,
        _ => 0,
    }
    return first + second + third
}
