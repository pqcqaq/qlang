async fn worker(value: Int) -> Int {
    return value
}

async fn make_tasks(base: Int) -> [Task[Int]; 2] {
    return [worker(base), worker(base + 2)]
}

async fn main() -> Int {
    var total = 0
    for await value in await make_tasks(20) {
        total = total + value
    }
    return total
}
