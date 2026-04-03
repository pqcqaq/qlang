struct Wrap {
    values: [Int; 0],
}

struct PendingWrap {
    tasks: [Task[Wrap]; 2],
}

struct PendingInt {
    tasks: [Task[Int]; 2],
    fallback: Task[Int],
}

struct Slot {
    value: Int,
}

async fn wrap_worker() -> Wrap {
    return Wrap { values: [] }
}

async fn int_worker(value: Int) -> Int {
    return value
}

async fn projected_reinit(index: Int) -> Wrap {
    var pending = PendingWrap {
        tasks: [wrap_worker(), wrap_worker()],
    }
    let slot = Slot { value: index }
    let first = await pending.tasks[slot.value]
    pending.tasks[slot.value] = wrap_worker()
    return await pending.tasks[slot.value]
}

async fn aliased_projected_reinit(index: Int) -> Wrap {
    var pending = PendingWrap {
        tasks: [wrap_worker(), wrap_worker()],
    }
    let slot = Slot { value: index }
    let alias = pending.tasks
    let first = await alias[slot.value]
    pending.tasks[index] = wrap_worker()
    return await alias[slot.value]
}

async fn composed_reinit(row: Int) -> Int {
    var tasks = [int_worker(1), int_worker(2)]
    let slots = [row, row]
    let first = await tasks[slots[row]]
    tasks[slots[row]] = int_worker(first + 1)
    return await tasks[slots[row]]
}

async fn alias_composed_reinit(row: Int) -> Int {
    var tasks = [int_worker(3), int_worker(4)]
    let slots = [row, row]
    let alias = slots
    let first = await tasks[alias[row]]
    tasks[slots[row]] = int_worker(first + 1)
    return await tasks[alias[row]]
}

async fn guard_refined_reinit(index: Int) -> Wrap {
    var tasks = [wrap_worker(), wrap_worker()]
    if index == 0 {
        let first = await tasks[index]
        tasks[0] = wrap_worker()
    }
    return await tasks[0]
}

async fn dynamic_assignment(index: Int) -> Int {
    var tasks = [int_worker(8), int_worker(9)]
    tasks[index] = int_worker(10)
    return await tasks[0]
}

async fn spawn_sibling(index: Int) -> Int {
    let pending = PendingInt {
        tasks: [int_worker(5), int_worker(6)],
        fallback: int_worker(7),
    }
    let running = spawn pending.tasks[index]
    let first = await running
    let second = await pending.fallback
    return first + second
}

async fn helper() -> Int {
    let first_wrap = await projected_reinit(0)
    let second_wrap = await aliased_projected_reinit(1)
    let third_wrap = await guard_refined_reinit(0)
    let first = await composed_reinit(0)
    let second = await alias_composed_reinit(0)
    let third = await dynamic_assignment(1)
    let fourth = await spawn_sibling(1)
    return first + second + third + fourth
}

extern "c" pub fn q_export() -> Int {
    return 1
}
