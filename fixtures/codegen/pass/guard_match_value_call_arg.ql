struct State {
    value: Int,
}

fn allow(state: State) -> Bool {
    return state.value == 1
}

fn main() -> Int {
    let ready = true
    return match 1 {
        1 if allow(match ready { true => State { value: 1 }, false => State { value: 2 } }) => 10,
        _ => 0,
    }
}
