async fn worker(value: Int) -> Int {
    return value
}

async fn make_batches(base: Int) -> [(Task[Int], Task[Int]); 1] {
    return [(worker(base), worker(base + 2))]
}

async fn main() -> Int {
    let batches = await make_batches(20)
    var total = 0
    for await value in batches[0] {
        total = total + value
    }
    return total
}
