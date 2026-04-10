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

    match (if branch { pair_value } else { alt_pair_value })() {
        (left, right) if left < right => sink(left + right),
        _ => sink(0),
    }

    match (match branch { true => state_value, false => alt_state_value })() {
        State { value } if value == 3 => sink(value),
        _ => sink(0),
    }

    match (if branch { values } else { alt_values })() {
        [first, middle, last] if middle == 5 => sink(first + middle + last),
        _ => sink(0),
    }

    defer match (match branch { true => pair_value, false => alt_pair_value })() {
        (left, right) if left < right => sink(left + right),
        _ => sink(0),
    }

    defer match (if branch { state_value } else { alt_state_value })() {
        State { value } if value == 3 => sink(value),
        _ => sink(0),
    }

    defer match (match branch { true => values, false => alt_values })() {
        [first, middle, last] if middle == 5 => sink(first + middle + last),
        _ => sink(0),
    }

    return 0
}
