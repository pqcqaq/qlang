use array_env as arrays
use tuple_env as tuples
use task_tuple_env as task_tuples
use deep_task_env as deep

struct ScalarArrayPayload {
    values: [Int; 2],
}

struct ScalarTuplePayload {
    values: (Int, Int),
}

struct TaskTuplePayload {
    values: (Task[Int], Task[Int]),
}

struct TaskArrayPayload {
    tasks: [Task[Int]; 2],
}

struct ScalarArrayEnvelope {
    payload: ScalarArrayPayload,
}

struct ScalarTupleEnvelope {
    payload: ScalarTuplePayload,
}

struct TaskTupleEnvelope {
    payload: TaskTuplePayload,
}

struct TaskArrayEnvelope {
    payload: TaskArrayPayload,
}

struct DeepTaskEnvelope {
    outer: TaskArrayEnvelope,
}

async fn worker(value: Int) -> Int {
    return value
}

fn array_env(base: Int) -> ScalarArrayEnvelope {
    return ScalarArrayEnvelope {
        payload: ScalarArrayPayload {
            values: [base, base + 1],
        },
    }
}

fn tuple_env(base: Int) -> ScalarTupleEnvelope {
    return ScalarTupleEnvelope {
        payload: ScalarTuplePayload {
            values: (base, base + 1),
        },
    }
}

fn task_tuple_env(base: Int) -> TaskTupleEnvelope {
    return TaskTupleEnvelope {
        payload: TaskTuplePayload {
            values: (worker(base), worker(base + 1)),
        },
    }
}

fn deep_task_env(base: Int) -> DeepTaskEnvelope {
    return DeepTaskEnvelope {
        outer: TaskArrayEnvelope {
            payload: TaskArrayPayload {
                tasks: [worker(base), worker(base + 1)],
            },
        },
    }
}

async fn main() -> Int {
    var total = 0
    for await value in arrays(8).payload.values {
        total = total + value
    }
    for await value in tuples(4).payload.values {
        total = total + value
    }
    for await value in task_tuples(2).payload.values {
        total = total + value
    }
    for await value in deep(5).outer.payload.tasks {
        total = total + value
    }
    return total
}
