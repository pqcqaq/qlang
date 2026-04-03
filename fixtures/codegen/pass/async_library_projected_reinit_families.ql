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

async fn literal_reinit() -> Wrap {
    var tasks = [worker(), worker()]
    let first = await tasks[0]
    tasks[0] = worker()
    return await tasks[0]
}

async fn literal_conditional_reinit(flag: Bool) -> Wrap {
    var tasks = [worker(), worker()]
    if flag {
        let first = await tasks[0]
        tasks[0] = worker()
    }
    return await tasks[0]
}

async fn projected_dynamic_reinit(index: Int) -> Wrap {
    var tasks = [worker(), worker()]
    let slot = Slot { value: index }
    let first = await tasks[slot.value]
    tasks[slot.value] = worker()
    return await tasks[slot.value]
}

async fn projected_dynamic_conditional_reinit(flag: Bool, index: Int) -> Wrap {
    var tasks = [worker(), worker()]
    let slot = Slot { value: index }
    if flag {
        let first = await tasks[slot.value]
        tasks[slot.value] = worker()
    }
    return await tasks[slot.value]
}

async fn projected_root_dynamic_reinit(index: Int) -> Wrap {
    var pending = Pending {
        tasks: [worker(), worker()],
    }
    let slot = Slot { value: index }
    let first = await pending.tasks[slot.value]
    pending.tasks[slot.value] = worker()
    return await pending.tasks[slot.value]
}

async fn helper() -> Wrap {
    let first = await literal_reinit()
    let second = await literal_conditional_reinit(true)
    let third = await projected_dynamic_reinit(0)
    let fourth = await projected_dynamic_conditional_reinit(true, 0)
    let fifth = await projected_root_dynamic_reinit(0)
    return fifth
}
