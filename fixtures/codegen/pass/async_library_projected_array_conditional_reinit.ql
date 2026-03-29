struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn helper(flag: Bool) -> Wrap {
    var tasks = [worker(), worker()]
    if flag {
        let first = await tasks[0]
        tasks[0] = worker()
    }
    return await tasks[0]
}
