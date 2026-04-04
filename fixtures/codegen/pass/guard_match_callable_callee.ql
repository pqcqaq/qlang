struct State {
    value: Int,
}

fn allow_one(state: State) -> Bool {
    return state.value == 1
}

fn allow_two(state: State) -> Bool {
    return state.value == 2
}

fn main() -> Int {
    let ready = true
    return match 1 {
        1 if (match ready { true => allow_one, false => allow_two })(State { value: 1 }) => 10,
        _ => 0,
    }
}
