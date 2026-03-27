async fn worker() -> Int {
    return 1
}

fn forward(task: Task[Int]) -> Task[Int] {
    return task
}

async fn helper() -> Int {
    let task = worker()
    let running = spawn forward(task)
    return await running
}
