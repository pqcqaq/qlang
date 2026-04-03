use SLOT as INDEX_ALIAS

struct Pending {
    tasks: [Task[Int]; 2],
}

struct Slot {
    value: Int,
}

static BASE: Int = 2
static SLOT: Slot = Slot { value: BASE - 2 }

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

    var helper_pending = Pending {
        tasks: [worker(2), worker(8)],
    }
    let helper_alias = helper_pending.tasks
    if INDEX_ALIAS.value == 0 {
        let helper_first = await helper_alias[alias_slots[row]]
        helper_pending.tasks[slots[row]] = worker(helper_first + 5)
    }
    let helper_forwarded = forward(helper_alias[alias_slots[row]])
    let helper_tail = await helper_pending.tasks[1]
    let helper_second = await helper_forwarded

    var queued_pending = Pending {
        tasks: [worker(3), worker(11)],
    }
    let queued_alias = queued_pending.tasks
    if INDEX_ALIAS.value == 0 {
        let queued_first = await queued_alias[alias_slots[row]]
        queued_pending.tasks[slots[row]] = worker(queued_first + 7)
    }
    let queued_tail_tasks = queued_pending.tasks
    let queued_forwarded = forward(queued_alias[alias_slots[row]])
    let queued = queued_forwarded
    let queued_tail = await queued_tail_tasks[1]
    let queued_running = spawn queued
    let queued_second = await queued_running

    return helper_second + helper_tail + queued_second + queued_tail
}
