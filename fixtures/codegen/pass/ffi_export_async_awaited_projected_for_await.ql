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

async fn array_env(base: Int) -> ScalarArrayEnvelope {
    return ScalarArrayEnvelope {
        payload: ScalarArrayPayload {
            values: [base, base + 1],
        },
    }
}

async fn tuple_env(base: Int) -> ScalarTupleEnvelope {
    return ScalarTupleEnvelope {
        payload: ScalarTuplePayload {
            values: (base, base + 1),
        },
    }
}

async fn task_tuple_env(base: Int) -> TaskTupleEnvelope {
    return TaskTupleEnvelope {
        payload: TaskTuplePayload {
            values: (worker(base), worker(base + 1)),
        },
    }
}

async fn deep_task_env(base: Int) -> DeepTaskEnvelope {
    return DeepTaskEnvelope {
        outer: TaskArrayEnvelope {
            payload: TaskArrayPayload {
                tasks: [worker(base), worker(base + 1)],
            },
        },
    }
}

async fn helper() -> Int {
    var total = 0
    for await value in (await array_env(1)).payload.values {
        total = total + value
    }
    for await value in (await tuple_env(3)).payload.values {
        total = total + value
    }
    for await value in (await task_tuple_env(5)).payload.values {
        total = total + value
    }
    for await value in (await deep_task_env(7)).outer.payload.tasks {
        total = total + value
    }
    return total
}

extern "c" pub fn q_export() -> Int {
    return 1
}
