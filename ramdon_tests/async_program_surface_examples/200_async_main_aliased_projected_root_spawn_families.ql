struct Pending {
    tasks: [Task[Int]; 2],
}

struct Slot {
    value: Int,
}

struct Bundle {
    left: Task[Int],
    right: Task[Int],
}

struct Envelope {
    bundle: Bundle,
    tail: Task[Int],
}

struct ArrayBundle {
    tasks: [Task[Int]; 2],
}

struct ArrayEnvelope {
    bundle: ArrayBundle,
    tail: Task[Int],
}

async fn worker(value: Int) -> Int {
    return value
}

fn forward(task: Task[Int]) -> Task[Int] {
    return task
}

async fn main() -> Int {
    let slot = Slot { value: 0 }

    var nested_pending = Pending {
        tasks: [worker(9), worker(14)],
    }
    let nested_alias = nested_pending.tasks
    let nested_first = await nested_alias[slot.value]
    nested_pending.tasks[slot.value] = worker(nested_first + 4)
    let nested_env = Envelope {
        bundle: Bundle {
            left: nested_alias[slot.value],
            right: worker(7),
        },
        tail: nested_pending.tasks[1],
    }
    let nested_running = spawn nested_env.bundle.left
    let nested_second = await nested_running
    let nested_extra = await nested_env.bundle.right
    let nested_tail = await nested_env.tail

    var array_pending = Pending {
        tasks: [worker(9), worker(14)],
    }
    let array_alias = array_pending.tasks
    let array_first = await array_alias[slot.value]
    array_pending.tasks[slot.value] = worker(array_first + 4)
    let array_tasks = [array_alias[slot.value], worker(10)]
    let array_running = spawn array_tasks[0]
    let array_second = await array_running
    let array_extra = await array_tasks[1]
    let array_tail = await array_pending.tasks[1]

    var nested_array_pending = Pending {
        tasks: [worker(9), worker(14)],
    }
    let nested_array_alias = nested_array_pending.tasks
    let nested_array_first = await nested_array_alias[slot.value]
    nested_array_pending.tasks[slot.value] = worker(nested_array_first + 6)
    let nested_array_env = ArrayEnvelope {
        bundle: ArrayBundle {
            tasks: [nested_array_alias[slot.value], worker(12)],
        },
        tail: nested_array_pending.tasks[1],
    }
    let nested_array_running = spawn nested_array_env.bundle.tasks[0]
    let nested_array_second = await nested_array_running
    let nested_array_extra = await nested_array_env.bundle.tasks[1]
    let nested_array_tail = await nested_array_env.tail

    var forwarded_pending = Pending {
        tasks: [worker(9), worker(14)],
    }
    let forwarded_alias = forwarded_pending.tasks
    let forwarded_first = await forwarded_alias[slot.value]
    forwarded_pending.tasks[slot.value] = worker(forwarded_first + 8)
    let forwarded_env = ArrayEnvelope {
        bundle: ArrayBundle {
            tasks: [forward(forwarded_alias[slot.value]), worker(21)],
        },
        tail: forwarded_pending.tasks[1],
    }
    let forwarded_running = spawn forwarded_env.bundle.tasks[0]
    let forwarded_second = await forwarded_running
    let forwarded_extra = await forwarded_env.bundle.tasks[1]
    let forwarded_tail = await forwarded_env.tail

    return nested_second
        + nested_extra
        + nested_tail
        + array_second
        + array_extra
        + array_tail
        + nested_array_second
        + nested_array_extra
        + nested_array_tail
        + forwarded_second
        + forwarded_extra
        + forwarded_tail
}
