use LIMITS as INPUT
use matches as check

static LIMITS: [Int; 3] = [4, 8, 9]

struct State {
    ready: Bool,
}

fn state(flag: Bool) -> State {
    return State { ready: flag }
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
    let first = match true {
        true if enabled(extra: INPUT[0] == bundle(3)[offset(3)], state: state(bundle(3)[offset(3)] == 4)) => 10,
        false => 0,
    }
    let second = match 3 {
        current if [bundle(current)[offset(current)], INPUT[1], INPUT[2]][0] == INPUT[0] => 12,
        _ => 0,
    }
    let third = match 3 {
        current if check(expected: INPUT[0], value: [bundle(current)[offset(current)], 8, 9][0]) => 20,
        _ => 0,
    }
    return first + second + third
}
