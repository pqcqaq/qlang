use pair_value as pair_alias
use state_value as state_alias
use values as values_alias
use PAIR_VALUE as pair_const_alias
use STATE_VALUE as state_const_alias
use VALUES as values_const_alias

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
    match await pair_alias(1) {
        (left, right) if left < right => sink(left + right),
        _ => sink(0),
    }

    match await state_const_alias(3) {
        State { value } if value == 3 => sink(value),
        _ => sink(0),
    }

    match await values_alias(4) {
        [first, middle, last] if middle == 5 => sink(first + middle + last),
        _ => sink(0),
    }

    defer match await pair_const_alias(4) {
        (left, right) if left < right => sink(left + right),
        _ => sink(0),
    }

    defer match await state_alias(6) {
        State { value } if value == 6 => sink(value),
        _ => sink(0),
    }

    defer match await values_const_alias(7) {
        [first, middle, last] if middle == 8 => sink(first + middle + last),
        _ => sink(0),
    }

    return 0
}
