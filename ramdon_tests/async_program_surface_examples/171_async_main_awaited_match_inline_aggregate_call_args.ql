struct State {
    ready: Bool,
}

async fn fetch_flag(value: Bool) -> Bool {
    return value
}

async fn fetch_value(value: Int) -> Int {
    return value
}

fn enabled(state: State) -> Bool {
    return state.ready
}

fn matches(pair: (Int, Int), expected: Int) -> Bool {
    return pair[1] == expected
}

fn contains(values: [Int; 3], expected: Int) -> Bool {
    return values[1] == expected
}

async fn main() -> Int {
    let first = await fetch_flag(value: true)
    let from_struct_arg = match first {
        true if enabled(State { ready: true }) => 10,
        false => 0,
    }

    let second = await fetch_value(value: 22)
    let from_tuple_arg = match second {
        current if matches((0, current), 22) => 12,
        _ => 0,
    }

    let third = await fetch_value(value: 3)
    let from_array_arg = match third {
        current if contains([current, current + 1, current + 2], 4) => 20,
        _ => 0,
    }

    return from_struct_arg + from_tuple_arg + from_array_arg
}
