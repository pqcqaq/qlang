use fetch_value as load_scalar
use load_state as load_pairs
use offset as shift
use matches as check
use pair as make_pair

struct Pair {
    left: Int,
    right: Int,
}

struct State {
    values: Pair,
}

async fn fetch_value(value: Int) -> Int {
    return value
}

async fn load_state(value: Int) -> State {
    return State {
        values: Pair {
            left: value,
            right: value + 2,
        },
    }
}

fn offset(delta: Int, value: Int) -> Int {
    return value + delta
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
    let first = await load_scalar(value: 20)
    let from_scalar = match first {
        current if shift(delta: 2, value: current) == 22 => 20,
        _ => 0,
    }

    let second = await load_pairs(value: 20)
    let from_projection = match second {
        current if check(expected: 22, value: current.values) => 20,
        _ => 0,
    }

    let third = await load_pairs(value: 20)
    let from_call_root = match third {
        current if check(expected: 22, value: make_pair(value: current.values.left)) => 22,
        _ => 0,
    }

    return from_scalar + from_projection + from_call_root
}
