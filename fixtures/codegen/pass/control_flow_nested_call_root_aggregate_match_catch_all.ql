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

fn tuple_env(base: Int) -> TupleEnvelope {
    return TupleEnvelope {
        payload: TuplePayload {
            values: (base, base + 1),
        },
    }
}

fn alt_tuple_env(base: Int) -> TupleEnvelope {
    return TupleEnvelope {
        payload: TuplePayload {
            values: (base + 2, base + 3),
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

fn alt_state_env(base: Int) -> StateEnvelope {
    return StateEnvelope {
        payload: StatePayload {
            current: State { value: base + 1 },
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

fn alt_deep_env(base: Int) -> DeepEnvelope {
    return DeepEnvelope {
        outer: ArrayEnvelope {
            payload: ArrayPayload {
                values: [base + 3, base + 4, base + 5],
            },
        },
    }
}

fn main() -> Int {
    match (if true { tuple_env } else { alt_tuple_env })(1).payload.values {
        (left, right) if left < right => sink(left + right),
        _ => sink(0),
    }

    match (match true { true => state_env, false => alt_state_env })(3).payload.current {
        State { value } if value == 3 => sink(value),
        _ => sink(0),
    }

    match (if true { deep_env } else { alt_deep_env })(4).outer.payload.values {
        [first, middle, last] if middle == 5 => sink(first + middle + last),
        _ => sink(0),
    }

    defer match (match true { true => tuple_env, false => alt_tuple_env })(1).payload.values {
        (left, right) if left < right => sink(left + right),
        _ => sink(0),
    }

    defer match (if true { state_env } else { alt_state_env })(3).payload.current {
        State { value } if value == 3 => sink(value),
        _ => sink(0),
    }

    defer match (match true { true => deep_env, false => alt_deep_env })(4).outer.payload.values {
        [first, middle, last] if middle == 5 => sink(first + middle + last),
        _ => sink(0),
    }

    return 0
}
