struct Wrap {
    values: [Int; 0],
}

struct Slot {
    value: Int,
}

struct Pending {
    tasks: [Task[Wrap]; 2],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn helper(index: Int) -> Wrap {
    var pending = Pending {
        tasks: [worker(), worker()],
    }
    let slot = Slot { value: index }
    let alias = pending.tasks
    let first = await alias[slot.value]
    pending.tasks[index] = worker()
    return await alias[slot.value]
}
