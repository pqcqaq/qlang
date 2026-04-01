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
    let env = Envelope {
        bundle: Bundle {
            left: alias[slot.value],
            right: worker(9),
        },
        tail: pending.tasks[1],
    }
    let second = await env.bundle.left
    let extra = await env.bundle.right
    let tail = await env.tail
    return second + extra + tail
}
