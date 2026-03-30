struct Wrap {
    values: [Int; 0],
}

struct Pending {
    first: Task[Wrap],
    second: Task[Wrap],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn outer() -> Pending {
    return Pending { first: worker(), second: worker() }
}

fn score(value: Wrap) -> Int {
    return 1
}

async fn main() -> Int {
    let pending = await outer()
    let first = await pending.first
    let second = await pending.second
    return score(first) + score(second)
}
