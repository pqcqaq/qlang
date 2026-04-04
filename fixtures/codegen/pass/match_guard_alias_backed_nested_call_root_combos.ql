use bundle as pack
use offset as slot
use ready as flag
use enabled as allow
use state as make
use matches as check
use seed as literal

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

fn enabled(state: State, extra: Bool) -> Bool {
    return state.ready && extra
}

fn matches(value: Int, expected: Int) -> Bool {
    return value == expected
}

fn seed(value: Int) -> Int {
    return value
}

fn main() -> Int {
    let first = match true {
        true if allow(extra: flag(pack(3)[slot(3)] == literal(4)), state: make(flag(pack(3)[slot(3)] == literal(4)))) => 10,
        false => 0,
    }
    let second = match 3 {
        current if [pack(current)[slot(current)], literal(8), literal(9)][0] == literal(4) => 12,
        _ => 0,
    }
    let third = match 3 {
        current if check(expected: literal(4), value: [pack(current)[slot(current)], literal(8), 9][0]) => 20,
        _ => 0,
    }
    return first + second + third
}
