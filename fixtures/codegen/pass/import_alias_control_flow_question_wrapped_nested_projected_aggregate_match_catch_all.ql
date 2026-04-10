use LEFT_TUPLE as left_tuple_alias
use RIGHT_TUPLE as right_tuple_alias
use LEFT_STATE as left_state_alias
use RIGHT_STATE as right_state_alias
use LEFT_ARRAY as left_array_alias
use RIGHT_ARRAY as right_array_alias
use CLEANUP_LEFT_TUPLE as cleanup_left_tuple_alias
use CLEANUP_RIGHT_TUPLE as cleanup_right_tuple_alias
use CLEANUP_LEFT_STATE as cleanup_left_state_alias
use CLEANUP_RIGHT_STATE as cleanup_right_state_alias
use CLEANUP_LEFT_ARRAY as cleanup_left_array_alias
use CLEANUP_RIGHT_ARRAY as cleanup_right_array_alias

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

const LEFT_TUPLE: TupleEnvelope = TupleEnvelope {
    outer: TupleOuter {
        payload: TuplePayload {
            values: (1, 2),
        },
    },
}

const RIGHT_TUPLE: TupleEnvelope = TupleEnvelope {
    outer: TupleOuter {
        payload: TuplePayload {
            values: (7, 8),
        },
    },
}

const LEFT_STATE: StateEnvelope = StateEnvelope {
    outer: StateOuter {
        payload: StatePayload {
            current: State { value: 3 },
        },
    },
}

const RIGHT_STATE: StateEnvelope = StateEnvelope {
    outer: StateOuter {
        payload: StatePayload {
            current: State { value: 9 },
        },
    },
}

const LEFT_ARRAY: ArrayEnvelope = ArrayEnvelope {
    outer: ArrayOuter {
        payload: ArrayPayload {
            values: [4, 5, 6],
        },
    },
}

const RIGHT_ARRAY: ArrayEnvelope = ArrayEnvelope {
    outer: ArrayOuter {
        payload: ArrayPayload {
            values: [10, 11, 12],
        },
    },
}

const CLEANUP_LEFT_TUPLE: TupleEnvelope = TupleEnvelope {
    outer: TupleOuter {
        payload: TuplePayload {
            values: (4, 5),
        },
    },
}

const CLEANUP_RIGHT_TUPLE: TupleEnvelope = TupleEnvelope {
    outer: TupleOuter {
        payload: TuplePayload {
            values: (13, 14),
        },
    },
}

const CLEANUP_LEFT_STATE: StateEnvelope = StateEnvelope {
    outer: StateOuter {
        payload: StatePayload {
            current: State { value: 6 },
        },
    },
}

const CLEANUP_RIGHT_STATE: StateEnvelope = StateEnvelope {
    outer: StateOuter {
        payload: StatePayload {
            current: State { value: 15 },
        },
    },
}

const CLEANUP_LEFT_ARRAY: ArrayEnvelope = ArrayEnvelope {
    outer: ArrayOuter {
        payload: ArrayPayload {
            values: [7, 8, 9],
        },
    },
}

const CLEANUP_RIGHT_ARRAY: ArrayEnvelope = ArrayEnvelope {
    outer: ArrayOuter {
        payload: ArrayPayload {
            values: [16, 17, 18],
        },
    },
}

fn main() -> Int {
    let branch = true

    match ((if branch { left_tuple_alias } else { right_tuple_alias })?).outer.payload.values {
        (left, right) if left < right => sink(left + right),
        _ => sink(0),
    }

    match ((match branch { true => left_state_alias, false => right_state_alias })?).outer.payload.current {
        State { value } if value == 3 => sink(value),
        _ => sink(0),
    }

    match ((if branch { left_array_alias } else { right_array_alias })?).outer.payload.values {
        [first, middle, last] if middle == 5 => sink(first + middle + last),
        _ => sink(0),
    }

    defer match ((match branch { true => cleanup_left_tuple_alias, false => cleanup_right_tuple_alias })?).outer.payload.values {
        (left, right) if left < right => sink(left + right),
        _ => sink(0),
    }

    defer match ((if branch { cleanup_left_state_alias } else { cleanup_right_state_alias })?).outer.payload.current {
        State { value } if value == 6 => sink(value),
        _ => sink(0),
    }

    defer match ((match branch { true => cleanup_left_array_alias, false => cleanup_right_array_alias })?).outer.payload.values {
        [first, middle, last] if middle == 8 => sink(first + middle + last),
        _ => sink(0),
    }

    return 0
}
