async fn worker() -> Int {
    return 1
}

async fn helper() -> Int {
    return await worker()
}

extern "c" pub fn q_add(left: Int, right: Int) -> Int {
    return left + right
}
