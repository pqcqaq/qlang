struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn helper(index: Int) -> Wrap {
    var tasks = [worker(), worker()]
    if index == 0 {
        let first = await tasks[index]
        tasks[0] = worker()
    }
    return await tasks[0]
}
