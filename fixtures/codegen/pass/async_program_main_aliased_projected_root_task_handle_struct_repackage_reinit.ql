struct Pending {
    tasks: [Task[Int]; 2],
}

struct Slot {
    value: Int,
}

struct Bundle {
    left: Task[Int],
    right: Task[Int],
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
    let bundle = Bundle {
        left: alias[slot.value],
        right: worker(6),
    }
    let second = await bundle.left
    let extra = await bundle.right
    let tail = await pending.tasks[1]
    return second + extra + tail
}
