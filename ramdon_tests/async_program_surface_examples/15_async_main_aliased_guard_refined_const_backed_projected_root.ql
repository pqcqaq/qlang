struct Pending {
    tasks: [Task[Int]; 2],
}

struct Slot {
    value: Int,
}

const INDEX: Int = 0

async fn worker(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    var pending = Pending {
        tasks: [worker(8), worker(13)],
    }
    let alias = pending.tasks
    let slot = Slot { value: INDEX }
    if slot.value == 0 {
        let first = await alias[slot.value]
        pending.tasks[0] = worker(first + 4)
    }
    let second = await alias[0]
    let tail = await pending.tasks[1]
    return second + tail
}
