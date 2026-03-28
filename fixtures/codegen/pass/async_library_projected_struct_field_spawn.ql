async fn worker() -> Int {
    return 1
}

struct Pair {
    task: Task[Int],
    value: Int,
}

async fn helper() -> Int {
    let pair = Pair { task: worker(), value: 1 }
    let running = spawn pair.task
    return await running
}
