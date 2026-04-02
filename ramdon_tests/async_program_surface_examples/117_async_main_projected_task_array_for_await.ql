struct Pending {
    tasks: [Task[Int]; 2],
}

async fn worker(value: Int) -> Int {
    return value
}

fn make_pending(base: Int) -> Pending {
    return Pending { tasks: [worker(base), worker(base + 2)] }
}

async fn main() -> Int {
    let pending = make_pending(20)
    var total = 0
    for await value in pending.tasks {
        total = total + value
    }
    return total
}
