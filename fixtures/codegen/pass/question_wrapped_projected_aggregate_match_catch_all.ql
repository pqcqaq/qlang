extern "c" fn sink(value: Int)

struct State {
    value: Int,
}

struct Bundle {
    pair: (Int, Int),
    current: State,
    values: [Int; 3],
}

fn bundle_value() -> Bundle {
    return Bundle {
        pair: (1, 2),
        current: State { value: 3 },
        values: [4, 5, 6],
    }
}

fn main() -> Int {
    match (bundle_value()?).pair {
        (left, right) if left < right => sink(left + right),
        _ => sink(0),
    }

    match (bundle_value()?).current {
        State { value } if value == 3 => sink(value),
        _ => sink(0),
    }

    match (bundle_value()?).values {
        [first, middle, last] if middle == 5 => sink(first + middle + last),
        _ => sink(0),
    }

    defer match (bundle_value()?).pair {
        (left, right) if left < right => sink(left + right),
        _ => sink(0),
    }

    defer match (bundle_value()?).current {
        State { value } if value == 3 => sink(value),
        _ => sink(0),
    }

    defer match (bundle_value()?).values {
        [first, middle, last] if middle == 5 => sink(first + middle + last),
        _ => sink(0),
    }

    return 0
}
