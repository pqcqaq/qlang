async fn worker() -> Int {
    return 1
}

async fn helper() -> Int {
    let task = worker()
    let running = spawn task
    return await running
}
