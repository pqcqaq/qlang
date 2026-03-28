struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn helper() -> Wrap {
    let task = worker()
    let running = spawn forward(task)
    return await running
}
