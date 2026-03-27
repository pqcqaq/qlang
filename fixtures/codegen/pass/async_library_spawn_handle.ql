async fn worker() -> Int {
    return 1
}

async fn helper() -> Int {
    let task = spawn worker()
    return await task
}
