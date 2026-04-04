use bundle as pack
use offset as slot
use matches as check
use ready as flag

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

fn ready(flag: Bool) -> Bool {
    return flag
}

fn seed(value: Int) -> Int {
    return value
}

fn enabled(state: State, extra: Bool) -> Bool {
    return state.ready && extra
}

fn matches(value: Int, expected: Int) -> Bool {
    return value == expected
}

fn main() -> Int {
    let first = match true {
        true if enabled(extra: flag(pack(3)[slot(3)] == 4), state: state(flag(pack(3)[slot(3)] == 4))) => 10,
        false => 0,
    }
    let second = match 3 {
        current if [pack(current)[slot(current)], seed(8), seed(9)][0] == seed(4) => 12,
        _ => 0,
    }
    let third = match 3 {
        current if check(expected: seed(4), value: [pack(current)[slot(current)], seed(8), 9][0]) => 20,
        _ => 0,
    }
    return first + second + third
}
