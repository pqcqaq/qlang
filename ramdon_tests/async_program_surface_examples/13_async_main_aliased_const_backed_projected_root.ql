struct Pending {
    tasks: [Task[Int]; 2],
}

const INDEX: Int = 0

async fn worker(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    var pending = Pending {
        tasks: [worker(6), worker(9)],
    }
    let alias = pending.tasks
    let first = await alias[INDEX]
    pending.tasks[0] = worker(first + 2)
    let second = await alias[INDEX]
    let tail = await pending.tasks[1]
    return second + tail
}
