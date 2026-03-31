async fn worker(value: Int) -> Int {
    return value
}

async fn helper(index: Int) -> Int {
    var tasks = [worker(1), worker(2)]
    tasks[index] = worker(3)
    return await tasks[0]
}
