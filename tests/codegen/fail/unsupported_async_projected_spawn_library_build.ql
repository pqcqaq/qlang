async fn worker() -> Int {
    return 1
}

async fn helper() -> Int {
    let pair = (worker(), 1)
    spawn pair[0]
    return 0
}
