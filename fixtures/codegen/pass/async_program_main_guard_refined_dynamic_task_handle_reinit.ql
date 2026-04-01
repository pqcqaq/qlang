async fn worker(value: Int) -> Int {
    return value
}

fn score(value: Int) -> Int {
    return value
}

async fn helper(index: Int) -> Int {
    var tasks = [worker(1), worker(2)]
    if index == 0 {
        let first = await tasks[index]
        tasks[0] = worker(first + 1)
    }
    let final_value = await tasks[0]
    return score(final_value)
}

async fn main() -> Int {
    return await helper(0)
}
