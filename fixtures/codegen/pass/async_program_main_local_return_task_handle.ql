async fn worker() -> Int {
    return 1
}

fn schedule() -> Task[Int] {
    let task = worker()
    return task
}

async fn main() -> Int {
    let first = await schedule()
    let second = await schedule()
    return first + second
}
