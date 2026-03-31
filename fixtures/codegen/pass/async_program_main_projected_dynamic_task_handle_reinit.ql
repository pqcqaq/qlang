struct Slot {
    value: Int,
}

async fn worker(value: Int) -> Int {
    return value
}

fn score(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    var tasks = [worker(1), worker(2)]
    let slot = Slot { value: 0 }
    let first = await tasks[slot.value]
    tasks[slot.value] = worker(first + 1)
    let second = await tasks[slot.value]
    return score(first) + score(second)
}
