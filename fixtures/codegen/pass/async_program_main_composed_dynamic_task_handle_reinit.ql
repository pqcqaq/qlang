async fn worker(value: Int) -> Int {
    return value
}

fn choose() -> Int {
    return 0
}

fn score(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    let row = choose()
    var tasks = [worker(1), worker(2)]
    let slots = [row, row]
    let first = await tasks[slots[row]]
    tasks[slots[row]] = worker(first + 1)
    let final_value = await tasks[slots[row]]
    return score(final_value)
}
