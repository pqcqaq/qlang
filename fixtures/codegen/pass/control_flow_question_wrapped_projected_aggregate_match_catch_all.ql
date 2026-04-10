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

fn alt_bundle_value() -> Bundle {
    return Bundle {
        pair: (7, 8),
        current: State { value: 9 },
        values: [10, 11, 12],
    }
}

fn main() -> Int {
    let branch = true

    match ((if branch { bundle_value } else { alt_bundle_value })()?).pair {
        (left, right) if left < right => sink(left + right),
        _ => sink(0),
    }

    match ((match branch { true => bundle_value, false => alt_bundle_value })()?).current {
        State { value } if value == 3 => sink(value),
        _ => sink(0),
    }

    match ((if branch { bundle_value } else { alt_bundle_value })()?).values {
        [first, middle, last] if middle == 5 => sink(first + middle + last),
        _ => sink(0),
    }

    defer match ((match branch { true => bundle_value, false => alt_bundle_value })()?).pair {
        (left, right) if left < right => sink(left + right),
        _ => sink(0),
    }

    defer match ((if branch { bundle_value } else { alt_bundle_value })()?).current {
        State { value } if value == 3 => sink(value),
        _ => sink(0),
    }

    defer match ((match branch { true => bundle_value, false => alt_bundle_value })()?).values {
        [first, middle, last] if middle == 5 => sink(first + middle + last),
        _ => sink(0),
    }

    return 0
}
