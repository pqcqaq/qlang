struct ArrayPayload {
    values: [Int; 2],
}

struct TuplePayload {
    values: (Int, Int),
}

struct DeepPayload {
    inner: ArrayPayload,
}

fn main() -> Int {
    var total = 0
    for value in (ArrayPayload { values: [10, 11] }).values {
        total = total + value
    }
    for value in (TuplePayload { values: (7, 8) }).values {
        total = total + value
    }
    for value in (DeepPayload {
        inner: ArrayPayload { values: [3, 3] },
    })
        .inner
        .values
    {
        total = total + value
    }
    return total
}
