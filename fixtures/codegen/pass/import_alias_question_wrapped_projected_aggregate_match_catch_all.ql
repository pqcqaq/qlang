use bundle_value as bundle_alias
use cleanup_bundle_value as cleanup_bundle_alias

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

fn cleanup_bundle_value() -> Bundle {
    return Bundle {
        pair: (4, 5),
        current: State { value: 6 },
        values: [7, 8, 9],
    }
}

fn main() -> Int {
    match (bundle_alias()?).pair {
        (left, right) if left < right => sink(left + right),
        _ => sink(0),
    }

    match (bundle_alias()?).current {
        State { value } if value == 3 => sink(value),
        _ => sink(0),
    }

    match (bundle_alias()?).values {
        [first, middle, last] if middle == 5 => sink(first + middle + last),
        _ => sink(0),
    }

    defer match (cleanup_bundle_alias()?).pair {
        (left, right) if left < right => sink(left + right),
        _ => sink(0),
    }

    defer match (cleanup_bundle_alias()?).current {
        State { value } if value == 6 => sink(value),
        _ => sink(0),
    }

    defer match (cleanup_bundle_alias()?).values {
        [first, middle, last] if middle == 8 => sink(first + middle + last),
        _ => sink(0),
    }

    return 0
}
