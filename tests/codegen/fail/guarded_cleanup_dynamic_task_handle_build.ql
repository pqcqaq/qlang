struct Wrap {
    values: [Int; 0],
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn helper(index: Int) -> Wrap {
    var tasks = [worker(), worker()]
    defer if index == 0 { forward(tasks[index]) } else { forward(worker()) }
    if index != 0 {
        return await tasks[0]
    };
    tasks[0] = worker()
    return await worker()
}
