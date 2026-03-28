async fn worker() -> Int {
    return 1
}

fn schedule() -> Task[Int] {
    let task = worker()
    return task
}

async fn helper() -> Int {
    return await schedule()
}
