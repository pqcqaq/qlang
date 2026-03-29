struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn helper(index: Int) -> Wrap {
    var tasks = [worker(), worker()]
    tasks[index] = worker()
    return await tasks[0]
}
