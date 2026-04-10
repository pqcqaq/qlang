use pair_value as pair_alias
use alt_pair_value as alt_pair_alias
use state_value as state_alias
use alt_state_value as alt_state_alias
use values as values_alias
use alt_values as alt_values_alias

extern "c" fn sink(value: Int)

struct State {
    value: Int,
}

fn pair_value() -> (Int, Int) {
    return (1, 2)
}

fn alt_pair_value() -> (Int, Int) {
    return (7, 8)
}

fn state_value() -> State {
    return State { value: 3 }
}

fn alt_state_value() -> State {
    return State { value: 9 }
}

fn values() -> [Int; 3] {
    return [4, 5, 6]
}

fn alt_values() -> [Int; 3] {
    return [10, 11, 12]
}

fn main() -> Int {
    let branch = true

    match (if branch { pair_alias } else { alt_pair_alias })()? {
        (left, right) if left < right => sink(left + right),
        _ => sink(0),
    }

    match (match branch { true => state_alias, false => alt_state_alias })()? {
        State { value } if value == 3 => sink(value),
        _ => sink(0),
    }

    match (if branch { values_alias } else { alt_values_alias })()? {
        [first, middle, last] if middle == 5 => sink(first + middle + last),
        _ => sink(0),
    }

    defer match (match branch { true => pair_alias, false => alt_pair_alias })()? {
        (left, right) if left < right => sink(left + right),
        _ => sink(0),
    }

    defer match (if branch { state_alias } else { alt_state_alias })()? {
        State { value } if value == 3 => sink(value),
        _ => sink(0),
    }

    defer match (match branch { true => values_alias, false => alt_values_alias })()? {
        [first, middle, last] if middle == 5 => sink(first + middle + last),
        _ => sink(0),
    }

    return 0
}
