struct Pending {
    tasks: [Task[Int]; 2],
}

const INDEX: Int = 0

async fn worker(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    var pending = Pending {
        tasks: [worker(8), worker(13)],
    }
    let first = await pending.tasks[INDEX]
    pending.tasks[0] = worker(first + 3)
    let second = await pending.tasks[INDEX]
    let tail = await pending.tasks[1]
    return second + tail
}
