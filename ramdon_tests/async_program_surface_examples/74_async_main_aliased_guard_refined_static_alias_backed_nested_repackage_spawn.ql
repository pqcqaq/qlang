use SLOT as INDEX_ALIAS

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

static SLOT: Slot = Slot { value: 0 }

async fn worker(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    var pending = Pending {
        tasks: [worker(9), worker(15)],
    }
    let alias = pending.tasks
    if INDEX_ALIAS.value == 0 {
        let first = await alias[INDEX_ALIAS.value]
        pending.tasks[0] = worker(first + 6)
    }
    let env = Envelope {
        bundle: Bundle {
            left: alias[INDEX_ALIAS.value],
            right: worker(12),
        },
        tail: pending.tasks[1],
    }
    let running = spawn env.bundle.left
    let second = await running
    let extra = await env.bundle.right
    let tail = await env.tail
    return second + extra + tail
}
