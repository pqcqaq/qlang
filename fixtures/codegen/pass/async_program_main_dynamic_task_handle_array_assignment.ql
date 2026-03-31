async fn worker(value: Int) -> Int {
    return value
}

fn score(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    var index = 0
    var tasks = [worker(1), worker(2)]
    tasks[index] = worker(3)
    let value = await tasks[0]
    return score(value)
}
