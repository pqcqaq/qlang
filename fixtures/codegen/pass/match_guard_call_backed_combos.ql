use values as items
use offset as slot

struct State {
    ready: Bool,
}

fn ready(flag: Bool) -> Bool {
    return flag
}

fn enabled(state: State, extra: Bool) -> Bool {
    return state.ready && extra
}

fn seed(value: Int) -> Int {
    return value
}

fn matches(pair: (Int, Int), expected: Int) -> Bool {
    return pair[1] == expected
}

fn values(seed: Int) -> [Int; 3] {
    return [seed, seed + 1, seed + 2]
}

fn offset(value: Int) -> Int {
    return value - 2
}

fn main() -> Int {
    let first = match true {
        true if enabled(extra: ready(true), state: State { ready: ready(true) }) => 10,
        false => 0,
    }
    let second = match 22 {
        current if matches((seed(0), current), 22) => 12,
        _ => 0,
    }
    let third = match 3 {
        current if items(current)[slot(current)] == 4 => 20,
        _ => 0,
    }
    return first + second + third
}
