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

fn schedule() -> Task[Wrap] {
    let task = worker()
    return task
}

async fn outer_task() -> Task[Wrap] {
    return worker()
}

async fn outer_pending() -> Pending {
    return Pending { first: worker(), second: worker() }
}

fn score(value: Wrap) -> Int {
    return 1
}

async fn main() -> Int {
    let first = await schedule()
    let second = await schedule()

    let next = await outer_task()
    let third = await next

    let pending = await outer_pending()
    let fourth = await pending.first
    let fifth = await pending.second

    return score(first)
        + score(second)
        + score(third)
        + score(fourth)
        + score(fifth)
}
