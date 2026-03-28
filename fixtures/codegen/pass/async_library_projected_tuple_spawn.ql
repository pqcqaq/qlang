async fn worker() -> Int {
    return 1
}

async fn helper() -> Int {
    let pair = (worker(), 1)
    let running = spawn pair[0]
    return await running
}
