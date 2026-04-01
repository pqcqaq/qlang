struct Slot {
    value: Int,
}

struct Pending {
    tasks: [Task[Int]; 2],
}

async fn worker(value: Int) -> Int {
    return value
}

async fn guard_refined(index: Int) -> Int {
    var tasks = [worker(10), worker(20)]
    if index == 0 {
        let first = await tasks[index]
        tasks[0] = worker(first + 5)
    }
    let refreshed = await tasks[0]
    let tail = await tasks[1]
    return refreshed + tail
}

async fn main() -> Int {
    var pending = Pending {
        tasks: [worker(3), worker(4)],
    }
    let slot = Slot { value: 0 }
    let first = await pending.tasks[slot.value]
    pending.tasks[slot.value] = worker(first + 7)
    let refreshed = await pending.tasks[slot.value]
    let helper_total = await guard_refined(0)
    let tail = await pending.tasks[1]
    return helper_total + refreshed + tail
}
