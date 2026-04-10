extern "c" fn sink(value: Int)

struct State {
    value: Int,
}

async fn pair_value(seed: Int) -> (Int, Int) {
    return (seed, seed + 1)
}

async fn state_value(seed: Int) -> State {
    return State { value: seed }
}

async fn values(seed: Int) -> [Int; 3] {
    return [seed, seed + 1, seed + 2]
}

async fn main() -> Int {
    match await pair_value(1) {
        (left, right) if left < right => sink(left + right),
        _ => sink(0),
    }

    match await state_value(3) {
        State { value } if value == 3 => sink(value),
        _ => sink(0),
    }

    match await values(4) {
        [first, middle, last] if middle == 5 => sink(first + middle + last),
        _ => sink(0),
    }

    defer match await pair_value(4) {
        (left, right) if left < right => sink(left + right),
        _ => sink(0),
    }

    defer match await state_value(6) {
        State { value } if value == 6 => sink(value),
        _ => sink(0),
    }

    defer match await values(7) {
        [first, middle, last] if middle == 8 => sink(first + middle + last),
        _ => sink(0),
    }

    return 0
}
