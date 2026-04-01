struct Slot {
    value: Int,
}

struct Pending {
    tasks: [Task[Int]; 2],
    fallback: Task[Int],
}

async fn worker(value: Int) -> Int {
    return value
}

fn choose() -> Int {
    return 0
}

async fn main() -> Int {
    var index = 0

    var tasks = [worker(1), worker(2)]
    tasks[index] = worker(3)
    let first = await tasks[0]

    var precise = [worker(4), worker(5)]
    let slot = Slot { value: 0 }
    let second = await precise[slot.value]
    precise[slot.value] = worker(second + 1)
    let third = await precise[slot.value]

    let pending = Pending {
        tasks: [worker(6), worker(7)],
        fallback: worker(8),
    }
    let running = spawn pending.tasks[index]
    let fourth = await running
    let fifth = await pending.fallback

    var projected = Pending {
        tasks: [worker(9), worker(10)],
        fallback: worker(11),
    }
    let sixth = await projected.tasks[slot.value]
    projected.tasks[slot.value] = worker(sixth + 1)
    let seventh = await projected.tasks[slot.value]

    let row = choose()
    var composed = [worker(12), worker(13)]
    let slots = [row, row]
    let alias = slots
    let eighth = await composed[alias[row]]
    composed[slots[row]] = worker(eighth + 1)
    let ninth = await composed[alias[row]]

    return first
        + second
        + third
        + fourth
        + fifth
        + sixth
        + seventh
        + eighth
        + ninth
}
