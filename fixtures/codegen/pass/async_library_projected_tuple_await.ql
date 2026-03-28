async fn worker() -> Int {
    return 1
}

async fn helper() -> Int {
    let pair = (worker(), 1)
    return await pair[0]
}
