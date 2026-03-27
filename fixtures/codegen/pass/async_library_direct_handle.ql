async fn worker() -> Int {
    return 1
}

async fn helper() -> Int {
    let task = worker()
    return await task
}
