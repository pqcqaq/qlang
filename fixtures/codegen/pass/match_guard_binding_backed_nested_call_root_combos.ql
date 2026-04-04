struct State {
    ready: Bool,
    value: Int,
}

fn state(flag: Bool, value: Int) -> State {
    return State { ready: flag, value: value }
}

fn bundle(seed: Int) -> [Int; 3] {
    return [seed, seed + 1, seed + 2]
}

fn offset(value: Int) -> Int {
    return value - 2
}

fn enabled(state: State, extra: Bool) -> Bool {
    return state.ready && extra
}

fn matches(value: Int, expected: Int) -> Bool {
    return value == expected
}

fn main() -> Int {
    let first = match state(flag: true, value: 3) {
        current if enabled(extra: bundle(current.value)[offset(current.value)] == 4, state: current) => 10,
        _ => 0,
    }
    let second = match state(flag: true, value: 3) {
        current if [bundle(current.value)[offset(current.value)], current.value + 5, 9][0] == 4 => 12,
        _ => 0,
    }
    let third = match state(flag: true, value: 3) {
        current if matches(expected: 4, value: [bundle(current.value)[offset(current.value)], current.value, 9][0]) => 20,
        _ => 0,
    }
    return first + second + third
}
