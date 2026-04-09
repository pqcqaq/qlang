extern "c" fn sink(value: Int)

struct ArrayPayload {
    values: [Int; 3],
}

struct TuplePayload {
    values: (Int, Int),
}

struct State {
    value: Int,
}

struct StatePayload {
    current: State,
}

struct ArrayEnvelope {
    payload: ArrayPayload,
}

struct TupleEnvelope {
    payload: TuplePayload,
}

struct StateEnvelope {
    payload: StatePayload,
}

struct DeepEnvelope {
    outer: ArrayEnvelope,
}

fn array_env(base: Int) -> ArrayEnvelope {
    return ArrayEnvelope {
        payload: ArrayPayload {
            values: [base, base + 1, base + 2],
        },
    }
}

fn tuple_env(base: Int) -> TupleEnvelope {
    return TupleEnvelope {
        payload: TuplePayload {
            values: (base, base + 1),
        },
    }
}

fn state_env(base: Int) -> StateEnvelope {
    return StateEnvelope {
        payload: StatePayload {
            current: State { value: base },
        },
    }
}

fn deep_env(base: Int) -> DeepEnvelope {
    return DeepEnvelope {
        outer: ArrayEnvelope {
            payload: ArrayPayload {
                values: [base, base + 1, base + 2],
            },
        },
    }
}

fn main() -> Int {
    match tuple_env(1).payload.values {
        (left, right) if left < right => sink(left + right),
        _ => sink(0),
    }

    match state_env(3).payload.current {
        State { value } if value == 3 => sink(value),
        _ => sink(0),
    }

    match array_env(4).payload.values {
        [first, middle, last] if middle == 5 => sink(first + middle + last),
        _ => sink(0),
    }

    match deep_env(6).outer.payload.values {
        [first, middle, last] if middle == 7 => sink(first + middle + last),
        _ => sink(0),
    }

    defer match tuple_env(1).payload.values {
        (left, right) if left < right => sink(left + right),
        _ => sink(0),
    }

    defer match state_env(3).payload.current {
        State { value } if value == 3 => sink(value),
        _ => sink(0),
    }

    defer match array_env(4).payload.values {
        [first, middle, last] if middle == 5 => sink(first + middle + last),
        _ => sink(0),
    }

    defer match deep_env(6).outer.payload.values {
        [first, middle, last] if middle == 7 => sink(first + middle + last),
        _ => sink(0),
    }

    return 0
}
