struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn schedule() -> Task[Wrap] {
    let task = worker()
    return task
}

fn score(value: Wrap) -> Int {
    return 1
}

async fn main() -> Int {
    let first = await schedule()
    let second = await schedule()
    return score(first) + score(second)
}
