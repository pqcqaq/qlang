async fn worker(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    var task = worker(1)
    let alias = task
    let first = await alias
    task = worker(first + 1)
    return await alias
}
