struct Pending {
    tasks: [Task[Int]; 2],
}

struct Bundle {
    left: Task[Int],
    right: Task[Int],
}

struct Envelope {
    bundle: Bundle,
    tail: Task[Int],
}

struct Slot {
    value: Int,
}

async fn worker(value: Int) -> Int {
    return value
}

async fn tuple_repackage(index: Int) -> Int {
    var pending = Pending {
        tasks: [worker(9), worker(14)],
    }
    let alias = pending.tasks
    let first = await alias[index]
    pending.tasks[index] = worker(first + 3)
    let pair = (alias[index], worker(5))
    let second = await pair[0]
    let extra = await pair[1]
    return second + extra
}

async fn struct_repackage() -> Int {
    var pending = Pending {
        tasks: [worker(11), worker(14)],
    }
    let slot = Slot { value: 0 }
    let alias = pending.tasks
    let first = await alias[slot.value]
    pending.tasks[slot.value] = worker(first + 4)
    let bundle = Bundle {
        left: alias[slot.value],
        right: worker(6),
    }
    let second = await bundle.left
    let extra = await bundle.right
    let tail = await pending.tasks[1]
    return second + extra + tail
}

async fn nested_repackage() -> Int {
    var pending = Pending {
        tasks: [worker(13), worker(17)],
    }
    let slot = Slot { value: 0 }
    let alias = pending.tasks
    let first = await alias[slot.value]
    pending.tasks[slot.value] = worker(first + 5)
    let env = Envelope {
        bundle: Bundle {
            left: alias[slot.value],
            right: worker(7),
        },
        tail: pending.tasks[1],
    }
    let second = await env.bundle.left
    let extra = await env.bundle.right
    let tail = await env.tail
    return second + extra + tail
}

async fn helper() -> Int {
    let first = await tuple_repackage(0)
    let second = await struct_repackage()
    let third = await nested_repackage()
    return first + second + third
}
