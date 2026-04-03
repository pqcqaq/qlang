use LIMITS as INPUT
use check as enabled

static LIMITS: [Int; 3] = [3, 4, 5]

struct State {
    ready: Bool,
    value: Int,
}

static READY: State = State { ready: true, value: 22 }

fn check(state: State, extra: Bool) -> Bool {
    return state.ready && extra
}

fn main() -> Int {
    let state = State { ready: true, value: 7 }
    let first = match true {
        true if enabled(extra: true, state: state) => 10,
        false => 0,
    }
    let second = match 22 {
        current if (INPUT[0], current)[1] == READY.value => 12,
        _ => 0,
    }
    let third = match 3 {
        current if [INPUT[0], current + 1, INPUT[2]][current - 2] == 4 => 20,
        _ => 0,
    }
    return first + second + third
}
