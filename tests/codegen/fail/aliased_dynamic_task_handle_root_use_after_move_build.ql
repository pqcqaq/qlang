struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn helper(index: Int) -> Wrap {
    let tasks = [worker(), worker()]
    let alias = tasks
    let first = await alias[index]
    return await tasks[index]
}
