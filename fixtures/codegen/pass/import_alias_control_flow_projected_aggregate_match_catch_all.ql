use LEFT_BUNDLE as left_bundle_alias
use RIGHT_BUNDLE as right_bundle_alias
use CLEANUP_LEFT_BUNDLE as cleanup_left_bundle_alias
use CLEANUP_RIGHT_BUNDLE as cleanup_right_bundle_alias

extern "c" fn sink(value: Int)

struct State {
    value: Int,
}

struct Bundle {
    pair: (Int, Int),
    current: State,
    values: [Int; 3],
}

const LEFT_BUNDLE: Bundle = Bundle {
    pair: (1, 2),
    current: State { value: 3 },
    values: [4, 5, 6],
}

const RIGHT_BUNDLE: Bundle = Bundle {
    pair: (7, 8),
    current: State { value: 9 },
    values: [10, 11, 12],
}

const CLEANUP_LEFT_BUNDLE: Bundle = Bundle {
    pair: (4, 5),
    current: State { value: 6 },
    values: [7, 8, 9],
}

const CLEANUP_RIGHT_BUNDLE: Bundle = Bundle {
    pair: (13, 14),
    current: State { value: 15 },
    values: [16, 17, 18],
}

fn main() -> Int {
    let branch = true

    match (if branch { left_bundle_alias } else { right_bundle_alias }).pair {
        (left, right) if left < right => sink(left + right),
        _ => sink(0),
    }

    match (match branch { true => left_bundle_alias, false => right_bundle_alias }).current {
        State { value } if value == 3 => sink(value),
        _ => sink(0),
    }

    match (if branch { left_bundle_alias } else { right_bundle_alias }).values {
        [first, middle, last] if middle == 5 => sink(first + middle + last),
        _ => sink(0),
    }

    defer match (match branch { true => cleanup_left_bundle_alias, false => cleanup_right_bundle_alias }).pair {
        (left, right) if left < right => sink(left + right),
        _ => sink(0),
    }

    defer match (if branch { cleanup_left_bundle_alias } else { cleanup_right_bundle_alias }).current {
        State { value } if value == 6 => sink(value),
        _ => sink(0),
    }

    defer match (match branch { true => cleanup_left_bundle_alias, false => cleanup_right_bundle_alias }).values {
        [first, middle, last] if middle == 8 => sink(first + middle + last),
        _ => sink(0),
    }

    return 0
}
