use make_pending as schedule

struct Pending {
    tasks: [Task[Int]; 2],
}

async fn worker(value: Int) -> Int {
    return value
}

async fn make_pending(base: Int) -> Pending {
    return Pending { tasks: [worker(base), worker(base + 2)] }
}

async fn main() -> Int {
    let pending = await schedule(base: 20)
    var total = 0
    for await value in pending.tasks {
        total = total + value
    }
    return total
}
