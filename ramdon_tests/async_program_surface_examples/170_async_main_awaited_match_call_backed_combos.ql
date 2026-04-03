use values as items
use offset as slot

struct State {
    ready: Bool,
}

async fn fetch_flag(value: Bool) -> Bool {
    return value
}

async fn fetch_value(value: Int) -> Int {
    return value
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

async fn main() -> Int {
    let first = await fetch_flag(value: true)
    let from_call_guard = match first {
        true if enabled(extra: ready(true), state: State { ready: ready(true) }) => 10,
        false => 0,
    }

    let second = await fetch_value(value: 22)
    let from_inline_tuple = match second {
        current if matches((seed(0), current), 22) => 12,
        _ => 0,
    }

    let third = await fetch_value(value: 3)
    let from_call_root = match third {
        current if items(current)[slot(current)] == 4 => 20,
        _ => 0,
    }

    return from_call_guard + from_inline_tuple + from_call_root
}
