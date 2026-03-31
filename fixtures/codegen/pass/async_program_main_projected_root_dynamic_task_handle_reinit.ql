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
        tasks: [worker(1), worker(2)],
    }
    let slot = Slot { value: 0 }
    let first = await pending.tasks[slot.value]
    pending.tasks[slot.value] = worker(first + 1)
    return await pending.tasks[slot.value]
}
