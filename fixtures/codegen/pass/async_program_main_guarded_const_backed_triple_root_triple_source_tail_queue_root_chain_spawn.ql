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

fn forward(task: Task[Int]) -> Task[Int] {
    return task
}

async fn main() -> Int {
    let row_root = INDEX
    let row = row_root
    let slots = [row, row]
    let slot_root = slots
    let slot_alias_root = slot_root
    let alias_slots = slot_alias_root
    var pending = Pending {
        tasks: [worker(8), worker(14)],
    }
    let root = pending.tasks
    let root_alias = root
    let alias = root_alias
    let slot = Slot { value: INDEX }
    let slot_alias = slot
    if slot_alias.value == 0 {
        let first = await alias[alias_slots[row]]
        pending.tasks[slots[row]] = worker(first + 24)
    }
    let tail_tasks = pending.tasks
    let forwarded = forward(alias[alias_slots[row]])
    let running_task = forwarded
    let env = Envelope {
        bundle: Bundle {
            tasks: [running_task, worker(42)],
        },
        tail: tail_tasks[1],
    }
    let queue_root = env.bundle.tasks
    let queue_alias_root = queue_root
    let queued_tasks = queue_alias_root
    let queued = queued_tasks[0]
    let running = spawn queued
    let second = await running
    let extra = await env.bundle.tasks[1]
    let tail = await env.tail
    return second + extra + tail
}
