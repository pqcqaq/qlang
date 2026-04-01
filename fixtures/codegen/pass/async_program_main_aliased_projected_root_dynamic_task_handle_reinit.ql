struct Pending {
    tasks: [Task[Int]; 2],
}

struct Slot {
    value: Int,
}

async fn worker(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    var pending = Pending {
        tasks: [worker(5), worker(8)],
    }
    let slot = Slot { value: 0 }
    let alias = pending.tasks
    let first = await alias[slot.value]
    pending.tasks[slot.value] = worker(first + 4)
    let second = await alias[slot.value]
    let tail = await pending.tasks[1]
    return second + tail
}
