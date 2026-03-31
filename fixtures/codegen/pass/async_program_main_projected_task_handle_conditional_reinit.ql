async fn worker(value: Int) -> Int {
    return value
}

fn score(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    let flag = true
    var tasks = [worker(1), worker(2)]
    if flag {
        let first = await tasks[0]
        tasks[0] = worker(7)
    }
    let final_value = await tasks[0]
    return score(final_value)
}
