struct Wrap {
    values: [Int; 0],
}

struct Pending {
    tasks: [Task[Wrap]; 2],
}

struct Slot {
    value: Int,
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn helper(index: Int) -> Wrap {
    var pending = Pending {
        tasks: [worker(), worker()],
    }
    let slot = Slot { value: index }
    let first = await pending.tasks[slot.value]
    pending.tasks[slot.value] = worker()
    return await pending.tasks[slot.value]
}
