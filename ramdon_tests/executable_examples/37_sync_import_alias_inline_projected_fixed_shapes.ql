use scalar as make_value

struct ArrayPayload {
    values: [Int; 2],
}

struct TuplePayload {
    values: (Int, Int),
}

struct DeepPayload {
    inner: ArrayPayload,
}

fn scalar(value: Int) -> Int {
    return value
}

fn main() -> Int {
    var total = 0
    for value in (ArrayPayload {
        values: [make_value(10), make_value(11)],
    })
        .values
    {
        total = total + value
    }
    for value in (TuplePayload {
        values: (make_value(7), make_value(8)),
    })
        .values
    {
        total = total + value
    }
    for value in (DeepPayload {
        inner: ArrayPayload {
            values: [make_value(3), make_value(3)],
        },
    })
        .inner
        .values
    {
        total = total + value
    }
    return total
}
