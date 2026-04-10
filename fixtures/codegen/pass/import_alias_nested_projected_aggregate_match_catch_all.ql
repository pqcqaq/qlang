use TUPLE_ENV as tuple_alias
use STATE_ENV as state_alias
use ARRAY_ENV as array_alias
use CLEANUP_TUPLE_ENV as cleanup_tuple_alias
use CLEANUP_STATE_ENV as cleanup_state_alias
use CLEANUP_ARRAY_ENV as cleanup_array_alias

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

const TUPLE_ENV: TupleEnvelope = TupleEnvelope {
    outer: TupleOuter {
        payload: TuplePayload {
            values: (1, 2),
        },
    },
}

const STATE_ENV: StateEnvelope = StateEnvelope {
    outer: StateOuter {
        payload: StatePayload {
            current: State { value: 3 },
        },
    },
}

const ARRAY_ENV: ArrayEnvelope = ArrayEnvelope {
    outer: ArrayOuter {
        payload: ArrayPayload {
            values: [4, 5, 6],
        },
    },
}

const CLEANUP_TUPLE_ENV: TupleEnvelope = TupleEnvelope {
    outer: TupleOuter {
        payload: TuplePayload {
            values: (4, 5),
        },
    },
}

const CLEANUP_STATE_ENV: StateEnvelope = StateEnvelope {
    outer: StateOuter {
        payload: StatePayload {
            current: State { value: 6 },
        },
    },
}

const CLEANUP_ARRAY_ENV: ArrayEnvelope = ArrayEnvelope {
    outer: ArrayOuter {
        payload: ArrayPayload {
            values: [7, 8, 9],
        },
    },
}

fn main() -> Int {
    match tuple_alias.outer.payload.values {
        (left, right) if left < right => sink(left + right),
        _ => sink(0),
    }

    match state_alias.outer.payload.current {
        State { value } if value == 3 => sink(value),
        _ => sink(0),
    }

    match array_alias.outer.payload.values {
        [first, middle, last] if middle == 5 => sink(first + middle + last),
        _ => sink(0),
    }

    defer match cleanup_tuple_alias.outer.payload.values {
        (left, right) if left < right => sink(left + right),
        _ => sink(0),
    }

    defer match cleanup_state_alias.outer.payload.current {
        State { value } if value == 6 => sink(value),
        _ => sink(0),
    }

    defer match cleanup_array_alias.outer.payload.values {
        [first, middle, last] if middle == 8 => sink(first + middle + last),
        _ => sink(0),
    }

    return 0
}
