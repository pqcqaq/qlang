use SLOT as INDEX_ALIAS

struct Pending {
    tasks: [Task[Int]; 2],
}

struct Slot {
    value: Int,
}

struct Bundle {
    tasks: [Task[Int]; 2],
}

struct Envelope {
    bundle: Bundle,
    tail: Task[Int],
}

static SLOT: Slot = Slot { value: 0 }

async fn worker(value: Int) -> Int {
    return value
}

fn forward(task: Task[Int]) -> Task[Int] {
    return task
}

async fn main() -> Int {
    var pending = Pending {
        tasks: [worker(10), worker(16)],
    }
    let alias = pending.tasks
    if INDEX_ALIAS.value == 0 {
        let first = await alias[INDEX_ALIAS.value]
        pending.tasks[0] = worker(first + 8)
    }
    let env = Envelope {
        bundle: Bundle {
            tasks: [forward(alias[INDEX_ALIAS.value]), worker(24)],
        },
        tail: pending.tasks[1],
    }
    let running = spawn env.bundle.tasks[0]
    let second = await running
    let extra = await env.bundle.tasks[1]
    let tail = await env.tail
    return second + extra + tail
}
