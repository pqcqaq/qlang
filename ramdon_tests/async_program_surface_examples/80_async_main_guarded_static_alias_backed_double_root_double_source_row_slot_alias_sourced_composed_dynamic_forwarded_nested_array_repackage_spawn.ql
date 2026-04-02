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
    let row_root = INDEX_ALIAS.value
    let row = row_root
    let slots = [row, row]
    let slot_root = slots
    let alias_slots = slot_root
    var pending = Pending {
        tasks: [worker(11), worker(17)],
    }
    let root = pending.tasks
    let alias = root
    let slot = Slot { value: INDEX_ALIAS.value }
    let slot_alias = slot
    if slot_alias.value == 0 {
        let first = await alias[alias_slots[row]]
        pending.tasks[slots[row]] = worker(first + 15)
    }
    let env = Envelope {
        bundle: Bundle {
            tasks: [forward(alias[alias_slots[row]]), worker(34)],
        },
        tail: pending.tasks[1],
    }
    let running = spawn env.bundle.tasks[0]
    let second = await running
    let extra = await env.bundle.tasks[1]
    let tail = await env.tail
    return second + extra + tail
}
