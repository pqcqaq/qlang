use LIMITS as INPUT
use check as enabled

static LIMITS: [Int; 3] = [3, 4, 5]

struct State {
    ready: Bool,
    value: Int,
}

static READY: State = State { ready: true, value: 22 }

async fn fetch_flag(value: Bool) -> Bool {
    return value
}

async fn fetch_value(value: Int) -> Int {
    return value
}

fn check(state: State, extra: Bool) -> Bool {
    return state.ready && extra
}

async fn main() -> Int {
    let first = await fetch_flag(value: true)
    let from_alias_guard = match first {
        true if enabled(extra: true, state: State { ready: true, value: 7 }) => 10,
        false => 0,
    }

    let second = await fetch_value(value: 22)
    let from_tuple_inline = match second {
        current if (INPUT[0], current)[1] == READY.value => 12,
        _ => 0,
    }

    let third = await fetch_value(value: 3)
    let from_array_inline = match third {
        current if [INPUT[0], current + 1, INPUT[2]][current - 2] == 4 => 20,
        _ => 0,
    }

    return from_alias_guard + from_tuple_inline + from_array_inline
}
