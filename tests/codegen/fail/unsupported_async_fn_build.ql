async fn worker() -> Int {
    return 1
}

async fn main() -> Int {
    return await worker()
}
