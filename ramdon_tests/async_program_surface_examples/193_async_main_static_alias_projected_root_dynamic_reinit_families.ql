use SLOT as INDEX_ALIAS

struct Pending {
    tasks: [Task[Int]; 2],
}

struct Slot {
    value: Int,
}

static SLOT: Slot = Slot { value: 0 }

async fn worker(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    var direct_pending = Pending {
        tasks: [worker(6), worker(9)],
    }
    let direct_alias = direct_pending.tasks
    let direct_first = await direct_alias[INDEX_ALIAS.value]
    direct_pending.tasks[0] = worker(direct_first + 2)
    let direct_second = await direct_alias[INDEX_ALIAS.value]
    let direct_tail = await direct_pending.tasks[1]

    var guarded_pending = Pending {
        tasks: [worker(8), worker(13)],
    }
    let guarded_alias = guarded_pending.tasks
    if INDEX_ALIAS.value == 0 {
        let guarded_first = await guarded_alias[INDEX_ALIAS.value]
        guarded_pending.tasks[0] = worker(guarded_first + 4)
    }
    let guarded_second = await guarded_alias[0]
    let guarded_tail = await guarded_pending.tasks[1]

    return direct_second + direct_tail + guarded_second + guarded_tail
}
