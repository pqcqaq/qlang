struct State {
    value: Int,
}

fn allow(state: State) -> Bool {
    return state.value == 1
}

fn main() -> Int {
    var state = State { value: 0 }
    return match 1 {
        1 if allow(state = State { value: 1 }) => state.value,
        _ => 0,
    }
}
