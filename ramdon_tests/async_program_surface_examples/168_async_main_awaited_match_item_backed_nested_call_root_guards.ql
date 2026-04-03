use LIMITS as INPUT
use matches as check

static LIMITS: [Int; 3] = [4, 8, 9]

struct State {
    ready: Bool,
}

async fn fetch_flag(value: Bool) -> Bool {
    return value
}

async fn fetch_value(value: Int) -> Int {
    return value
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

async fn main() -> Int {
    let first = await fetch_flag(value: true)
    let from_bool = match first {
        true if enabled(extra: INPUT[0] == bundle(3)[offset(3)], state: state(bundle(3)[offset(3)] == 4)) => 10,
        false => 0,
    }

    let second = await fetch_value(value: 3)
    let from_inline = match second {
        current if [bundle(current)[offset(current)], INPUT[1], INPUT[2]][0] == INPUT[0] => 12,
        _ => 0,
    }

    let third = await fetch_value(value: 3)
    let from_guard_call = match third {
        current if check(expected: INPUT[0], value: [bundle(current)[offset(current)], 8, 9][0]) => 20,
        _ => 0,
    }

    return from_bool + from_inline + from_guard_call
}
