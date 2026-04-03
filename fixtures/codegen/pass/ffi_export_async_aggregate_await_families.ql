struct Pair {
    left: Bool,
    right: Int,
}

struct Numbers {
    left: Int,
    right: Int,
}

struct TaskHolder {
    task: Task[Int],
    value: Int,
}

struct Wrap {
    values: [Int; 0],
}

async fn scalar_worker() -> Int {
    return 1
}

async fn tuple_worker() -> (Bool, Int) {
    return (true, 2)
}

async fn struct_worker() -> Pair {
    return Pair { left: true, right: 3 }
}

async fn array_worker() -> [Int; 3] {
    return [4, 5, 6]
}

async fn nested_worker() -> (Numbers, [Int; 2]) {
    return (Numbers { left: 7, right: 8 }, [9, 10])
}

async fn int_task_worker(value: Int) -> Int {
    return value
}

async fn wrap_worker() -> Wrap {
    return Wrap { values: [] }
}

async fn helper() -> Int {
    let scalar = await scalar_worker()

    let tuple = await tuple_worker()
    let tuple_value = tuple[1]

    let pair = await struct_worker()
    let pair_value = pair.right

    let array = await array_worker()
    let array_total = array[0] + array[1] + array[2]

    let nested = await nested_worker()
    let nested_total = nested[0].left + nested[0].right + nested[1][0] + nested[1][1]

    let tuple_tasks = (int_task_worker(11), 0)
    let tuple_task_value = await tuple_tasks[0]

    let tuple_spawn_tasks = (int_task_worker(12), 0)
    let tuple_running = spawn tuple_spawn_tasks[0]
    let tuple_spawn_value = await tuple_running

    let wrap_tasks = [wrap_worker(), wrap_worker()]
    let wrapped = await wrap_tasks[0]

    let holder = TaskHolder {
        task: int_task_worker(13),
        value: 14,
    }
    let holder_value = await holder.task

    let spawn_holder = TaskHolder {
        task: int_task_worker(15),
        value: 16,
    }
    let holder_running = spawn spawn_holder.task
    let holder_spawn_value = await holder_running

    return scalar
        + tuple_value
        + pair_value
        + array_total
        + nested_total
        + tuple_task_value
        + tuple_spawn_value
        + holder_value
        + holder_spawn_value
}

extern "c" pub fn q_export() -> Int {
    return 1
}
