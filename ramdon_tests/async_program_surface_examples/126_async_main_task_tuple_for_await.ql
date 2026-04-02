async fn worker(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    var total = 0
    for await value in (worker(20), worker(22)) {
        total = total + value
    }
    return total
}
