async fn worker() -> Int {
    return 1
}

struct Pair {
    task: Task[Int],
    value: Int,
}

async fn helper() -> Int {
    let pair = Pair { task: worker(), value: 1 }
    return await pair.task
}
