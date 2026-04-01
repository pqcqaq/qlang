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

struct Envelope {
    bundle: Bundle,
    tail: Task[Int],
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
    pending.tasks[slot.value] = worker(first + 4)
    let env = Envelope {
        bundle: Bundle {
            left: alias[slot.value],
            right: worker(7),
        },
        tail: pending.tasks[1],
    }
    let running = spawn env.bundle.left
    let second = await running
    let extra = await env.bundle.right
    let tail = await env.tail
    return second + extra + tail
}
