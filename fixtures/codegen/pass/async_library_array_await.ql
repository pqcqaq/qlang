async fn worker() -> [Int; 3] {
    return [1, 2, 3]
}

async fn helper() -> [Int; 3] {
    return await worker()
}
