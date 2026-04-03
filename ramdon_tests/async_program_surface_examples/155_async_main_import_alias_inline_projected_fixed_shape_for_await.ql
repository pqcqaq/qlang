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

async fn main() -> Int {
    var total = 0
    for await value in (ScalarArrayPayload {
        values: [make_value(8), make_value(9)],
    })
        .values
    {
        total = total + value
    }
    for await value in (ScalarTuplePayload {
        values: (make_value(4), make_value(5)),
    })
        .values
    {
        total = total + value
    }
    for await value in (TaskTuplePayload {
        values: (run(2), run(3)),
    })
        .values
    {
        total = total + value
    }
    for await value in (DeepPending {
        outer: PendingEnvelope {
            payload: Pending {
                tasks: [run(5), run(6)],
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
