use SLOT as INDEX_ALIAS

struct Pending {
    tasks: [Task[Int]; 2],
}

struct Bundle {
    tasks: [Task[Int]; 2],
}

struct Envelope {
    bundle: Bundle,
    tail: Task[Int],
}

struct Slot {
    value: Int,
}

static SLOT: Slot = Slot { value: 0 }

async fn worker(value: Int) -> Int {
    return value
}

fn forward(task: Task[Int]) -> Task[Int] {
    return task
}

async fn main() -> Int {
    let row = INDEX_ALIAS.value
    let slots = [row, row]
    let alias_slots = slots
    var pending = Pending {
        tasks: [worker(11), worker(17)],
    }
    let alias = pending.tasks
    if INDEX_ALIAS.value == 0 {
        let first = await alias[alias_slots[row]]
        pending.tasks[slots[row]] = worker(first + 10)
    }
    let env = Envelope {
        bundle: Bundle {
            tasks: [forward(alias[alias_slots[row]]), worker(27)],
        },
        tail: pending.tasks[1],
    }
    let running = spawn env.bundle.tasks[0]
    let second = await running
    let extra = await env.bundle.tasks[1]
    let tail = await env.tail
    return second + extra + tail
}
