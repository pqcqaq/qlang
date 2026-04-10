use pair_value as pair_alias
use state_value as state_alias
use values as values_alias

extern "c" fn sink(value: Int)

struct State {
    value: Int,
}

fn pair_value() -> (Int, Int) {
    return (1, 2)
}

fn state_value() -> State {
    return State { value: 3 }
}

fn values() -> [Int; 3] {
    return [4, 5, 6]
}

fn main() -> Int {
    match pair_alias()? {
        (left, right) if left < right => sink(left + right),
        _ => sink(0),
    }

    match state_alias()? {
        State { value } if value == 3 => sink(value),
        _ => sink(0),
    }

    match values_alias()? {
        [first, middle, last] if middle == 5 => sink(first + middle + last),
        _ => sink(0),
    }

    defer match pair_alias()? {
        (left, right) if left < right => sink(left + right),
        _ => sink(0),
    }

    defer match state_alias()? {
        State { value } if value == 3 => sink(value),
        _ => sink(0),
    }

    defer match values_alias()? {
        [first, middle, last] if middle == 5 => sink(first + middle + last),
        _ => sink(0),
    }

    return 0
}
