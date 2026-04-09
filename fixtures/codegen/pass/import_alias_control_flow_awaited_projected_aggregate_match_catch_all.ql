use tuple_env as tuple_alias
use state_env as state_alias
use array_env as array_alias
use TUPLE_ENV as tuple_const_alias
use STATE_ENV as state_const_alias
use ARRAY_ENV as array_const_alias

extern "c" fn sink(value: Int)

struct TuplePayload {
    values: (Int, Int),
}

struct State {
    value: Int,
}

struct StatePayload {
    current: State,
}

struct ArrayPayload {
    values: [Int; 3],
}

struct TupleEnvelope {
    payload: TuplePayload,
}

struct StateEnvelope {
    payload: StatePayload,
}

struct ArrayEnvelope {
    payload: ArrayPayload,
}

async fn tuple_env(base: Int) -> TupleEnvelope {
    return TupleEnvelope {
        payload: TuplePayload {
            values: (base, base + 1),
        },
    }
}

async fn state_env(base: Int) -> StateEnvelope {
    return StateEnvelope {
        payload: StatePayload {
            current: State { value: base },
        },
    }
}

async fn array_env(base: Int) -> ArrayEnvelope {
    return ArrayEnvelope {
        payload: ArrayPayload {
            values: [base, base + 1, base + 2],
        },
    }
}

const TUPLE_ENV: (Int) -> Task[TupleEnvelope] = tuple_env
const STATE_ENV: (Int) -> Task[StateEnvelope] = state_env
const ARRAY_ENV: (Int) -> Task[ArrayEnvelope] = array_env

async fn main() -> Int {
    let branch = true

    match (await (if branch { tuple_alias } else { tuple_const_alias })(1)).payload.values {
        (left, right) if left < right => sink(left + right),
        _ => sink(0),
    }

    match (await (match branch { true => state_const_alias, false => state_alias })(3)).payload.current {
        State { value } if value == 3 => sink(value),
        _ => sink(0),
    }

    match (await (if branch { array_alias } else { array_const_alias })(4)).payload.values {
        [first, middle, last] if middle == 5 => sink(first + middle + last),
        _ => sink(0),
    }

    defer match (await (match branch { true => tuple_const_alias, false => tuple_alias })(1)).payload.values {
        (left, right) if left < right => sink(left + right),
        _ => sink(0),
    }

    defer match (await (if branch { state_alias } else { state_const_alias })(3)).payload.current {
        State { value } if value == 3 => sink(value),
        _ => sink(0),
    }

    defer match (await (match branch { true => array_const_alias, false => array_alias })(4)).payload.values {
        [first, middle, last] if middle == 5 => sink(first + middle + last),
        _ => sink(0),
    }

    return 0
}
