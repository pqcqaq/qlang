struct Pending {
    tasks: [Task[Int]; 2],
    fallback: Task[Int],
}

async fn worker(value: Int) -> Int {
    return value
}

fn score(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    var index = 0
    let pending = Pending {
        tasks: [worker(1), worker(2)],
        fallback: worker(7),
    }
    let running = spawn pending.tasks[index]
    let first = await running
    let second = await pending.fallback
    return score(first) + score(second)
}
