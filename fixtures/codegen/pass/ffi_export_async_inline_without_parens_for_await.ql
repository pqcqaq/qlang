use scalar as make_value
use worker as run

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

fn scalar(value: Int) -> Int {
    return value
}

async fn worker(value: Int) -> Int {
    return value
}

async fn helper() -> Int {
    var total = 0
    for await value in ScalarArrayPayload { values: [8, 9] }.values {
        total = total + value
    }
    for await value in ScalarTuplePayload { values: (4, 5) }.values {
        total = total + value
    }
    for await value in TaskTuplePayload {
        values: (worker(2), worker(3)),
    }
        .values
    {
        total = total + value
    }
    for await value in DeepPending {
        outer: PendingEnvelope {
            payload: Pending {
                tasks: [worker(5), worker(6)],
            },
        },
    }
        .outer
        .payload
        .tasks
    {
        total = total + value
    }
    for await value in ScalarArrayPayload {
        values: [make_value(10), make_value(11)],
    }
        .values
    {
        total = total + value
    }
    for await value in ScalarTuplePayload {
        values: (make_value(12), make_value(13)),
    }
        .values
    {
        total = total + value
    }
    for await value in TaskTuplePayload {
        values: (run(14), run(15)),
    }
        .values
    {
        total = total + value
    }
    for await value in DeepPending {
        outer: PendingEnvelope {
            payload: Pending {
                tasks: [run(16), run(17)],
            },
        },
    }
        .outer
        .payload
        .tasks
    {
        total = total + value
    }
    return total
}

extern "c" pub fn q_export() -> Int {
    return 1
}
