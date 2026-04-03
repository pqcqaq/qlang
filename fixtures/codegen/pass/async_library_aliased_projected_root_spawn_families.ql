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

struct TaskBundle {
    tasks: [Task[Int]; 2],
}

struct Envelope {
    bundle: Bundle,
    tail: Task[Int],
}

struct ArrayEnvelope {
    bundle: TaskBundle,
    tail: Task[Int],
}

async fn worker(value: Int) -> Int {
    return value
}

fn forward(task: Task[Int]) -> Task[Int] {
    return task
}

async fn nested_repackage_spawn() -> Int {
    var pending = Pending {
        tasks: [worker(9), worker(14)],
    }
    let slot = Slot { value: 0 }
    let alias = pending.tasks
    let first = await alias[slot.value]
    pending.tasks[slot.value] = worker(first + 4)
    let env = Envelope {
        bundle: Bundle {
            left: alias[slot.value],
            right: worker(7),
        },
        tail: pending.tasks[1],
    }
    let running = spawn env.bundle.left
    let second = await running
    let extra = await env.bundle.right
    let tail = await env.tail
    return second + extra + tail
}

async fn array_repackage_spawn() -> Int {
    var pending = Pending {
        tasks: [worker(11), worker(15)],
    }
    let slot = Slot { value: 0 }
    let alias = pending.tasks
    let first = await alias[slot.value]
    pending.tasks[slot.value] = worker(first + 4)
    let tasks = [alias[slot.value], worker(10)]
    let running = spawn tasks[0]
    let second = await running
    let extra = await tasks[1]
    let tail = await pending.tasks[1]
    return second + extra + tail
}

async fn nested_array_repackage_spawn() -> Int {
    var pending = Pending {
        tasks: [worker(13), worker(17)],
    }
    let slot = Slot { value: 0 }
    let alias = pending.tasks
    let first = await alias[slot.value]
    pending.tasks[slot.value] = worker(first + 6)
    let env = ArrayEnvelope {
        bundle: TaskBundle {
            tasks: [alias[slot.value], worker(12)],
        },
        tail: pending.tasks[1],
    }
    let running = spawn env.bundle.tasks[0]
    let second = await running
    let extra = await env.bundle.tasks[1]
    let tail = await env.tail
    return second + extra + tail
}

async fn forwarded_nested_array_repackage_spawn() -> Int {
    var pending = Pending {
        tasks: [worker(19), worker(23)],
    }
    let slot = Slot { value: 0 }
    let alias = pending.tasks
    let first = await alias[slot.value]
    pending.tasks[slot.value] = worker(first + 8)
    let env = ArrayEnvelope {
        bundle: TaskBundle {
            tasks: [forward(alias[slot.value]), worker(21)],
        },
        tail: pending.tasks[1],
    }
    let running = spawn env.bundle.tasks[0]
    let second = await running
    let extra = await env.bundle.tasks[1]
    let tail = await env.tail
    return second + extra + tail
}

async fn helper() -> Int {
    let first = await nested_repackage_spawn()
    let second = await array_repackage_spawn()
    let third = await nested_array_repackage_spawn()
    let fourth = await forwarded_nested_array_repackage_spawn()
    return first + second + third + fourth
}
