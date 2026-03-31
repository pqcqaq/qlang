async fn worker(value: Int) -> Int {
    return value
}

async fn helper(row: Int) -> Int {
    var tasks = [worker(1), worker(2)]
    let slots = [row, row]
    let first = await tasks[slots[row]]
    tasks[slots[row]] = worker(first + 1)
    return await tasks[slots[row]]
}
