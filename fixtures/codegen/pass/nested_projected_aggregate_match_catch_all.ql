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
    let tuple_env = TupleEnvelope {
        outer: TupleOuter {
            payload: TuplePayload {
                values: (1, 2),
            },
        },
    }
    let state_env = StateEnvelope {
        outer: StateOuter {
            payload: StatePayload {
                current: State { value: 3 },
            },
        },
    }
    let array_env = ArrayEnvelope {
        outer: ArrayOuter {
            payload: ArrayPayload {
                values: [4, 5, 6],
            },
        },
    }

    match tuple_env.outer.payload.values {
        (left, right) if left < right => sink(left + right),
        _ => sink(0),
    }

    match state_env.outer.payload.current {
        State { value } if value == 3 => sink(value),
        _ => sink(0),
    }

    match array_env.outer.payload.values {
        [first, middle, last] if middle == 5 => sink(first + middle + last),
        _ => sink(0),
    }

    defer match tuple_env.outer.payload.values {
        (left, right) if left < right => sink(left + right),
        _ => sink(0),
    }

    defer match state_env.outer.payload.current {
        State { value } if value == 3 => sink(value),
        _ => sink(0),
    }

    defer match array_env.outer.payload.values {
        [first, middle, last] if middle == 5 => sink(first + middle + last),
        _ => sink(0),
    }

    return 0
}
