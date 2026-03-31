struct Wrap {
    values: [Int; 0],
}

struct Pending {
    tasks: [Task[Wrap]; 2],
}

const INDEX: Int = 0

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn helper() -> Wrap {
    let pending = Pending {
        tasks: [worker(), worker()],
    }
    let first = await pending.tasks[INDEX]
    return await pending.tasks[0]
}
