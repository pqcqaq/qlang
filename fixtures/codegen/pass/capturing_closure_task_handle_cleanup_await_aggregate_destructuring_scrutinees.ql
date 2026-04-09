struct Slot {
    value: Int,
}

struct State {
    slot: Slot,
}

extern "c" fn sink(value: Int)

async fn load_state(value: Int) -> State {
    return State { slot: Slot { value: value } }
}

async fn load_pair(value: Int) -> (Int, Int) {
    return (value, value + 1)
}

async fn main() -> Int {
    let branch = true
    let state_left_task = spawn load_state(13)
    let state_right_task = spawn load_state(7)
    let pair_left_task = spawn load_pair(20)
    let pair_right_task = spawn load_pair(1)

    let state_left = () => state_left_task
    let state_right = () => state_right_task
    let pair_left = () => pair_left_task
    let pair_right = () => pair_right_task

    defer {
        sink(match await (if branch { pair_left } else { pair_right })() {
            (left, right) => left + right,
        });
    }

    defer match await (match branch { true => state_left, false => state_right })() {
        State { slot: Slot { value } } if value == 13 => sink(value),
        _ => sink(0),
    }
    return 0
}
