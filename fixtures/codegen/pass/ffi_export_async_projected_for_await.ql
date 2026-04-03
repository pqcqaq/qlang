struct ScalarArrayPayload {
    values: [Int; 2],
}

struct ScalarTuplePayload {
    values: (Int, Int),
}

struct TaskArrayPayload {
    tasks: [Task[Int]; 2],
}

struct TaskTuplePayload {
    tasks: (Task[Int], Task[Int]),
}

async fn worker(value: Int) -> Int {
    return value
}

async fn helper() -> Int {
    var total = 0
    let arrays = ScalarArrayPayload { values: [1, 2] }
    for await value in arrays.values {
        total = total + value
    }
    let tuples = ScalarTuplePayload { values: (3, 4) }
    for await value in tuples.values {
        total = total + value
    }
    let task_arrays = TaskArrayPayload { tasks: [worker(5), worker(6)] }
    for await value in task_arrays.tasks {
        total = total + value
    }
    let task_tuples = TaskTuplePayload { tasks: (worker(7), worker(8)) }
    for await value in task_tuples.tasks {
        total = total + value
    }
    return total
}

extern "c" pub fn q_export() -> Int {
    return 1
}
