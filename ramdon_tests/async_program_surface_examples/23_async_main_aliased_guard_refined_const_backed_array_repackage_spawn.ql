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
        tasks: [worker(8), worker(14)],
    }
    let alias = pending.tasks
    let slot = Slot { value: INDEX }
    if slot.value == 0 {
        let first = await alias[slot.value]
        pending.tasks[0] = worker(first + 5)
    }
    let tasks = [alias[slot.value], worker(13)]
    let running = spawn tasks[0]
    let second = await running
    let extra = await tasks[1]
    let tail = await pending.tasks[1]
    return second + extra + tail
}
