struct Pending {
    tasks: [Task[Int]; 2],
}

async fn worker(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    var index = 1
    var pending = Pending { tasks: [worker(3), worker(4)] }
    pending.tasks[index] = worker(9)
    let left = await pending.tasks[0]
    let right = await pending.tasks[1]
    return left + right
}
