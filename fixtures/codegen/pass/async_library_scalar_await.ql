async fn worker() -> Int {
    return 1
}

async fn helper() -> Int {
    return await worker()
}
