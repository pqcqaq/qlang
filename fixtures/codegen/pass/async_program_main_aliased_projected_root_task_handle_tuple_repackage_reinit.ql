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
        tasks: [worker(9), worker(14)],
    }
    let slot = Slot { value: 0 }
    let alias = pending.tasks
    let first = await alias[slot.value]
    pending.tasks[slot.value] = worker(first + 3)
    let pair = (alias[slot.value], worker(5))
    let second = await pair[0]
    let extra = await pair[1]
    let tail = await pending.tasks[1]
    return second + extra + tail
}
