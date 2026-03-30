struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn other() -> Wrap {
    return Wrap { values: [] }
}

fn schedule() -> Task[Wrap] {
    return worker()
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

fn score(value: Wrap) -> Int {
    return 1
}

async fn main() -> Int {
    let direct = await schedule()

    let bound = schedule()
    let bound_value = await bound

    let spawned = spawn schedule()
    let spawned_value = await spawned

    let task = other()
    let forwarded = forward(task)
    let forwarded_value = await forwarded

    let next = worker()
    let running = spawn forward(next)
    let running_value = await running

    return score(direct)
        + score(bound_value)
        + score(spawned_value)
        + score(forwarded_value)
        + score(running_value)
}
