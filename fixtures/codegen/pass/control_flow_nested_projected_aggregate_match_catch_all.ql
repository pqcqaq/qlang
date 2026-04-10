extern "c" fn sink(value: Int)

struct TuplePayload {
    values: (Int, Int),
}

struct TupleOuter {
    payload: TuplePayload,
}

struct TupleEnvelope {
    outer: TupleOuter,
}

struct State {
    value: Int,
}

struct StatePayload {
    current: State,
}

struct StateOuter {
    payload: StatePayload,
}

struct StateEnvelope {
    outer: StateOuter,
}

struct ArrayPayload {
    values: [Int; 3],
}

struct ArrayOuter {
    payload: ArrayPayload,
}

struct ArrayEnvelope {
    outer: ArrayOuter,
}

fn main() -> Int {
    let tuple_left = TupleEnvelope {
        outer: TupleOuter {
            payload: TuplePayload {
                values: (1, 2),
            },
        },
    }
    let tuple_right = TupleEnvelope {
        outer: TupleOuter {
            payload: TuplePayload {
                values: (7, 8),
            },
        },
    }
    let state_left = StateEnvelope {
        outer: StateOuter {
            payload: StatePayload {
                current: State { value: 3 },
            },
        },
    }
    let state_right = StateEnvelope {
        outer: StateOuter {
            payload: StatePayload {
                current: State { value: 9 },
            },
        },
    }
    let array_left = ArrayEnvelope {
        outer: ArrayOuter {
            payload: ArrayPayload {
                values: [4, 5, 6],
            },
        },
    }
    let array_right = ArrayEnvelope {
        outer: ArrayOuter {
            payload: ArrayPayload {
                values: [10, 11, 12],
            },
        },
    }
    let branch = true

    match (if branch { tuple_left } else { tuple_right }).outer.payload.values {
        (left, right) if left < right => sink(left + right),
        _ => sink(0),
    }

    match (match branch { true => state_left, false => state_right }).outer.payload.current {
        State { value } if value == 3 => sink(value),
        _ => sink(0),
    }

    match (if branch { array_left } else { array_right }).outer.payload.values {
        [first, middle, last] if middle == 5 => sink(first + middle + last),
        _ => sink(0),
    }

    defer match (match branch { true => tuple_left, false => tuple_right }).outer.payload.values {
        (left, right) if left < right => sink(left + right),
        _ => sink(0),
    }

    defer match (if branch { state_left } else { state_right }).outer.payload.current {
        State { value } if value == 3 => sink(value),
        _ => sink(0),
    }

    defer match (match branch { true => array_left, false => array_right }).outer.payload.values {
        [first, middle, last] if middle == 5 => sink(first + middle + last),
        _ => sink(0),
    }

    return 0
}
