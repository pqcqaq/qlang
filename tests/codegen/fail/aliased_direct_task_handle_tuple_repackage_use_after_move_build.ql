struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn helper() -> Wrap {
    let task = worker()
    let alias = task
    let first = await task
    let pair = (alias, worker())
    return await pair[0]
}
