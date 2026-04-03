struct ScalarArrayPayload {
    values: [Int; 2],
}

struct ScalarTuplePayload {
    values: (Int, Int),
}

struct TaskTuplePayload {
    values: (Task[Int], Task[Int]),
}

struct Pending {
    tasks: [Task[Int]; 2],
}

struct PendingEnvelope {
    payload: Pending,
}

struct DeepPending {
    outer: PendingEnvelope,
}

async fn worker(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    var total = 0
    for await value in (ScalarArrayPayload { values: [8, 9] }).values {
        total = total + value
    }
    for await value in (ScalarTuplePayload { values: (4, 5) }).values {
        total = total + value
    }
    for await value in (TaskTuplePayload {
        values: (worker(2), worker(3)),
    })
        .values
    {
        total = total + value
    }
    for await value in (DeepPending {
        outer: PendingEnvelope {
            payload: Pending {
                tasks: [worker(5), worker(6)],
            },
        },
    })
        .outer
        .payload
        .tasks
    {
        total = total + value
    }
    return total
}
