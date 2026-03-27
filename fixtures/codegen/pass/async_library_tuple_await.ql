async fn worker() -> (Bool, Int) {
    return (true, 1)
}

async fn helper() -> (Bool, Int) {
    return await worker()
}
