extern "c" fn sink(value: Int)

struct State {
    value: Int,
}

fn main() -> Int {
    let current = State { value: 3 }
    let cleanup_current = State { value: 6 }

    match (1, 2) {
        (left, right) if left < right => sink(left + right),
        _ => sink(0),
    }

    match current {
        State { value } if value == 3 => sink(value),
        _ => sink(0),
    }

    defer match (4, 5) {
        (left, right) if left < right => sink(left + right),
        _ => sink(0),
    }

    defer match cleanup_current {
        State { value } if value == 6 => sink(value),
        _ => sink(0),
    }

    return 0
}
