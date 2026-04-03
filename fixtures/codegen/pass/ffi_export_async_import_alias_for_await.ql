use array_values as values
use tuple_values as pairs
use task_values as tasks
use make_pending as pending
use array_env as arrays
use tuple_env as tuples
use task_tuple_env as task_tuples
use deep_task_env as deep

struct Pending {
    tasks: [Task[Int]; 2],
}

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

fn array_values(base: Int) -> [Int; 2] {
    return [base, base + 1]
}

fn tuple_values(base: Int) -> (Int, Int) {
    return (base, base + 1)
}

fn task_values(base: Int) -> (Task[Int], Task[Int]) {
    return (worker(base), worker(base + 1))
}

fn make_pending(base: Int) -> Pending {
    return Pending {
        tasks: [worker(base), worker(base + 1)],
    }
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

async fn helper() -> Int {
    var total = 0
    for await value in values(1) {
        total = total + value
    }
    for await value in pairs(3) {
        total = total + value
    }
    for await value in tasks(5) {
        total = total + value
    }
    for await value in pending(7).tasks {
        total = total + value
    }
    for await value in arrays(9).payload.values {
        total = total + value
    }
    for await value in tuples(11).payload.values {
        total = total + value
    }
    for await value in task_tuples(13).payload.values {
        total = total + value
    }
    for await value in deep(15).outer.payload.tasks {
        total = total + value
    }
    return total
}

extern "c" pub fn q_export() -> Int {
    return 1
}
