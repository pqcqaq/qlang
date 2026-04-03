struct Pair {
    left: Int,
    right: Int,
}

struct State {
    values: Pair,
}

async fn load_state(value: Int) -> State {
    return State {
        values: Pair {
            left: value,
            right: value + 2,
        },
    }
}

fn pair(value: Int) -> Pair {
    return Pair {
        left: value,
        right: value + 2,
    }
}

fn matches(expected: Int, value: Pair) -> Bool {
    return value.right == expected
}

async fn main() -> Int {
    let first = await load_state(value: 20)
    let from_projection = match first {
        current if matches(expected: 22, value: current.values) => 20,
        _ => 0,
    }

    let second = await load_state(value: 20)
    let from_call_root = match second {
        current if matches(expected: 22, value: pair(value: current.values.left)) => 22,
        _ => 0,
    }

    return from_projection + from_call_root
}
