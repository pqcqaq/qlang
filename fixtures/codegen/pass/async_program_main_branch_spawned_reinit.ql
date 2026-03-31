async fn worker() -> Int {
    return 1
}

async fn fresh_worker() -> Int {
    return 2
}

fn score(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    let flag = true
    var task = worker()
    if flag {
        let running = spawn task
        task = fresh_worker()
        let first = await running
        return score(first)
    } else {
        task = fresh_worker()
    }
    let final_value = await task
    return score(final_value)
}
