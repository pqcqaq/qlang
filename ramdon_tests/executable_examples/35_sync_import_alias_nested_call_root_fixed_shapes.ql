use array_env as arrays
use tuple_env as tuples
use deep_env as deep

struct ArrayPayload {
    values: [Int; 2],
}

struct TuplePayload {
    values: (Int, Int),
}

struct ArrayEnvelope {
    payload: ArrayPayload,
}

struct TupleEnvelope {
    payload: TuplePayload,
}

struct DeepEnvelope {
    outer: ArrayEnvelope,
}

fn array_env(base: Int) -> ArrayEnvelope {
    return ArrayEnvelope {
        payload: ArrayPayload {
            values: [base, base],
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

fn deep_env(base: Int) -> DeepEnvelope {
    return DeepEnvelope {
        outer: ArrayEnvelope {
            payload: ArrayPayload {
                values: [base, base + 1],
            },
        },
    }
}

fn main() -> Int {
    var total = 0
    for value in arrays(10).payload.values {
        total = total + value
    }
    for value in tuples(7).payload.values {
        total = total + value
    }
    for value in deep(3).outer.payload.values {
        total = total + value
    }
    return total
}
