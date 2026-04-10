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

const PAIR_VALUE: (Int) -> Task[(Int, Int)] = pair_value
const STATE_VALUE: (Int) -> Task[State] = state_value
const VALUES: (Int) -> Task[[Int; 3]] = values

async fn main() -> Int {
    let branch = true

    match await (if branch { pair_value } else { PAIR_VALUE })(1) {
        (left, right) if left < right => sink(left + right),
        _ => sink(0),
    }

    match await (match branch { true => STATE_VALUE, false => state_value })(3) {
        State { value } if value == 3 => sink(value),
        _ => sink(0),
    }

    match await (if branch { values } else { VALUES })(4) {
        [first, middle, last] if middle == 5 => sink(first + middle + last),
        _ => sink(0),
    }

    defer match await (match branch { true => PAIR_VALUE, false => pair_value })(4) {
        (left, right) if left < right => sink(left + right),
        _ => sink(0),
    }

    defer match await (if branch { state_value } else { STATE_VALUE })(6) {
        State { value } if value == 6 => sink(value),
        _ => sink(0),
    }

    defer match await (match branch { true => VALUES, false => values })(7) {
        [first, middle, last] if middle == 8 => sink(first + middle + last),
        _ => sink(0),
    }

    return 0
}
