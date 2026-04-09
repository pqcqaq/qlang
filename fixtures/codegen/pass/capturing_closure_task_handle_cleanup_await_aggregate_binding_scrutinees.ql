struct Slot {
    ready: Bool,
    value: Int,
}

struct State {
    slot: Slot,
}

extern "c" fn sink(value: Int)

async fn load_state(value: Int) -> State {
    return State { slot: Slot { ready: true, value: value } }
}

async fn load_pair(value: Int) -> (Int, Int) {
    return (value, value + 1)
}

async fn load_values(value: Int) -> [Int; 3] {
    return [value, value + 1, value + 2]
}

async fn main() -> Int {
    let branch = true
    let state_left_task = spawn load_state(13)
    let state_right_task = spawn load_state(7)
    let pair_left_task = spawn load_pair(20)
    let pair_right_task = spawn load_pair(1)
    let values_left_task = spawn load_values(30)
    let values_right_task = spawn load_values(2)

    let state_left = () => state_left_task
    let state_right = () => state_right_task
    let pair_left = () => pair_left_task
    let pair_right = () => pair_right_task
    let values_left = () => values_left_task
    let values_right = () => values_right_task

    defer match await (if branch { state_left } else { state_right })() {
        current => sink(current.slot.value),
    }

    defer match await (match branch { true => pair_left, false => pair_right })() {
        current => sink(current[0] + current[1]),
    }

    defer match await (if branch { values_left } else { values_right })() {
        current => sink(current[0] + current[2]),
    }
    return 0
}
