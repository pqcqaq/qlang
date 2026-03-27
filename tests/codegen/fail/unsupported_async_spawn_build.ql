async fn worker() -> Int {
    return 1
}

async fn main() -> Int {
    spawn worker()
    return 0
}
