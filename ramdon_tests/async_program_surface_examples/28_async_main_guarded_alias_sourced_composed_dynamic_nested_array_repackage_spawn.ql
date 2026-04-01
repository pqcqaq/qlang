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

const INDEX: Int = 0

async fn worker(value: Int) -> Int {
    return value
}

fn choose() -> Int {
    return INDEX
}

async fn main() -> Int {
    let row = choose()
    let slots = [row, row]
    let alias_slots = slots
    var pending = Pending {
        tasks: [worker(8), worker(14)],
    }
    let alias = pending.tasks
    let slot = Slot { value: INDEX }
    if slot.value == 0 {
        let first = await alias[alias_slots[row]]
        pending.tasks[slots[row]] = worker(first + 8)
    }
    let env = Envelope {
        bundle: Bundle {
            tasks: [alias[alias_slots[row]], worker(20)],
        },
        tail: pending.tasks[1],
    }
    let running = spawn env.bundle.tasks[0]
    let second = await running
    let extra = await env.bundle.tasks[1]
    let tail = await env.tail
    return second + extra + tail
}
