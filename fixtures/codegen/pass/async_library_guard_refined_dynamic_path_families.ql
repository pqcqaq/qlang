use SLOT as INDEX_ALIAS

struct Pending {
    tasks: [Task[Int]; 2],
}

struct Slot {
    value: Int,
}

const INDEX: Int = 0

static SLOT: Slot = Slot { value: 0 }

async fn worker(value: Int) -> Int {
    return value
}

async fn direct_guard_refined() -> Int {
    var tasks = [worker(1), worker(2)]
    let index = 0
    if index == 0 {
        let first = await tasks[index]
        tasks[0] = worker(first + 1)
    }
    return await tasks[0]
}

async fn projected_guard_refined() -> Int {
    var tasks = [worker(3), worker(4)]
    let slot = Slot { value: 0 }
    if slot.value == 0 {
        let first = await tasks[slot.value]
        tasks[0] = worker(first + 2)
    }
    return await tasks[0]
}

async fn aliased_projected_root_guard_refined() -> Int {
    var pending = Pending {
        tasks: [worker(7), worker(11)],
    }
    let slot = Slot { value: 0 }
    let alias = pending.tasks
    if slot.value == 0 {
        let first = await alias[slot.value]
        pending.tasks[0] = worker(first + 3)
    }
    let second = await alias[0]
    let tail = await pending.tasks[1]
    return second + tail
}

async fn const_backed_aliased_projected_root_guard_refined() -> Int {
    var pending = Pending {
        tasks: [worker(8), worker(13)],
    }
    let alias = pending.tasks
    let slot = Slot { value: INDEX }
    if slot.value == 0 {
        let first = await alias[slot.value]
        pending.tasks[0] = worker(first + 4)
    }
    let second = await alias[0]
    let tail = await pending.tasks[1]
    return second + tail
}

async fn static_alias_backed_aliased_projected_root_guard_refined() -> Int {
    var pending = Pending {
        tasks: [worker(9), worker(15)],
    }
    let alias = pending.tasks
    if INDEX_ALIAS.value == 0 {
        let first = await alias[INDEX_ALIAS.value]
        pending.tasks[0] = worker(first + 5)
    }
    let second = await alias[0]
    let tail = await pending.tasks[1]
    return second + tail
}

async fn helper() -> Int {
    let first = await direct_guard_refined()
    let second = await projected_guard_refined()
    let third = await aliased_projected_root_guard_refined()
    let fourth = await const_backed_aliased_projected_root_guard_refined()
    let fifth = await static_alias_backed_aliased_projected_root_guard_refined()
    return first + second + third + fourth + fifth
}
