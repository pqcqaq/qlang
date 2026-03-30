struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn fresh_worker() -> Wrap {
    return Wrap { values: [] }
}

fn score(value: Wrap) -> Int {
    return 1
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
