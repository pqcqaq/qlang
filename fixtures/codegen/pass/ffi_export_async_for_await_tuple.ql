extern "c" pub fn q_export() -> Int {
    return 1
}

async fn worker() -> Int {
    return 1
}

async fn helper() -> Int {
    for await value in (1, 2, 3) {
        break
    }
    return await worker()
}
