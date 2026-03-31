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
    let flag = true
    var tasks = [worker(1), worker(2)]
    let slot = Slot { value: 0 }
    if flag {
        let first = await tasks[slot.value]
        tasks[slot.value] = worker(first + 1)
    }
    let final_value = await tasks[slot.value]
    return score(final_value)
}
