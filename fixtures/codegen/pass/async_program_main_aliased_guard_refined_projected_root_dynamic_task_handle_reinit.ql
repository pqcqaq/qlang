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
        tasks: [worker(7), worker(11)],
    }
    let slot = Slot { value: 0 }
    let alias = pending.tasks
    if slot.value == 0 {
        let first = await alias[slot.value]
        pending.tasks[0] = worker(first + 3)
    }
    let second = await alias[0]
    let tail = await pending.tasks[1]
    return second + tail
}
