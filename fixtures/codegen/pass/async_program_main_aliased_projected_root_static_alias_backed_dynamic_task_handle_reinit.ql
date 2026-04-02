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
    var pending = Pending {
        tasks: [worker(6), worker(9)],
    }
    let alias = pending.tasks
    let first = await alias[INDEX_ALIAS.value]
    pending.tasks[0] = worker(first + 2)
    let second = await alias[INDEX_ALIAS.value]
    let tail = await pending.tasks[1]
    return second + tail
}
