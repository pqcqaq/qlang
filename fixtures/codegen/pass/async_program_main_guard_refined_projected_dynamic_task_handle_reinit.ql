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
    if slot.value == 0 {
        let first = await tasks[slot.value]
        tasks[0] = worker(first + 1)
    }
    let final_value = await tasks[0]
    return score(final_value)
}
