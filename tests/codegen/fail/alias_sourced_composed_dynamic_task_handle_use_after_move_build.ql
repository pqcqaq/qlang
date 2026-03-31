async fn worker(value: Int) -> Int {
    return value
}

async fn helper(row: Int) -> Int {
    let tasks = [worker(1), worker(2)]
    let slots = [row, row]
    let alias = slots
    let first = await tasks[alias[row]]
    return await tasks[slots[row]]
}
