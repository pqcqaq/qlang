struct Wrap {
    values: [Int; 0],
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
    let alias = pending.tasks
    let first = await pending.tasks[index]
    pending.tasks[index] = worker()
    let pair = (alias[index], worker())
    return await pair[0]
}
