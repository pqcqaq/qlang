async fn worker() -> Int {
    return 1
}

async fn helper() -> Int {
    spawn worker()
    return 0
}
