struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn choose(flag: Bool, task: Task[Wrap]) -> Wrap {
    if flag {
        let running = spawn task
        return await running
    }
    return await task
}

async fn choose_reverse(flag: Bool, task: Task[Wrap]) -> Wrap {
    if flag {
        return await task
    }
    let running = spawn task
    return await running
}

async fn helper(flag: Bool) -> Wrap {
    return await choose(flag, worker())
}

async fn helper_reverse(flag: Bool) -> Wrap {
    return await choose_reverse(flag, worker())
}

fn score(value: Wrap) -> Int {
    return 1
}

async fn main() -> Int {
    let first = await helper(true)
    let second = await helper_reverse(false)
    return score(first) + score(second)
}
