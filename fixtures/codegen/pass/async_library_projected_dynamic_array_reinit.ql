struct Wrap {
    values: [Int; 0],
}

struct Slot {
    value: Int,
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn helper(index: Int) -> Wrap {
    var tasks = [worker(), worker()]
    let slot = Slot { value: index }
    let first = await tasks[slot.value]
    tasks[slot.value] = worker()
    return await tasks[slot.value]
}
