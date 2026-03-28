async fn worker() -> Int {
    return 1
}

fn schedule() -> Task[Int] {
    return worker()
}

async fn helper() -> Int {
    let task = schedule()
    let running = spawn task
    return await running
}
