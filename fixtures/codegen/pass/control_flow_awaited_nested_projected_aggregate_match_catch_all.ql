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

async fn tuple_env(base: Int) -> TupleEnvelope {
    return TupleEnvelope {
        outer: TupleOuter {
            payload: TuplePayload {
                values: (base, base + 1),
            },
        },
    }
}

async fn state_env(base: Int) -> StateEnvelope {
    return StateEnvelope {
        outer: StateOuter {
            payload: StatePayload {
                current: State { value: base },
            },
        },
    }
}

async fn array_env(base: Int) -> ArrayEnvelope {
    return ArrayEnvelope {
        outer: ArrayOuter {
            payload: ArrayPayload {
                values: [base, base + 1, base + 2],
            },
        },
    }
}

const TUPLE_ENV: (Int) -> Task[TupleEnvelope] = tuple_env
const STATE_ENV: (Int) -> Task[StateEnvelope] = state_env
const ARRAY_ENV: (Int) -> Task[ArrayEnvelope] = array_env

async fn main() -> Int {
    let branch = true

    match (await (if branch { tuple_env } else { TUPLE_ENV })(1)).outer.payload.values {
        (left, right) if left < right => sink(left + right),
        _ => sink(0),
    }

    match (await (match branch { true => STATE_ENV, false => state_env })(3)).outer.payload.current {
        State { value } if value == 3 => sink(value),
        _ => sink(0),
    }

    match (await (if branch { array_env } else { ARRAY_ENV })(4)).outer.payload.values {
        [first, middle, last] if middle == 5 => sink(first + middle + last),
        _ => sink(0),
    }

    defer match (await (match branch { true => TUPLE_ENV, false => tuple_env })(1)).outer.payload.values {
        (left, right) if left < right => sink(left + right),
        _ => sink(0),
    }

    defer match (await (if branch { state_env } else { STATE_ENV })(3)).outer.payload.current {
        State { value } if value == 3 => sink(value),
        _ => sink(0),
    }

    defer match (await (match branch { true => ARRAY_ENV, false => array_env })(4)).outer.payload.values {
        [first, middle, last] if middle == 5 => sink(first + middle + last),
        _ => sink(0),
    }

    return 0
}
