async fn worker() -> Int {
    return 1
}

async fn choose(flag: Bool, task: Task[Int]) -> Int {
    if flag {
        let running = spawn task
        return await running
    }
    return await task
}

async fn choose_reverse(flag: Bool, task: Task[Int]) -> Int {
    if flag {
        return await task
    }
    let running = spawn task
    return await running
}

async fn helper(flag: Bool) -> Int {
    return await choose(flag, worker())
}

async fn helper_reverse(flag: Bool) -> Int {
    return await choose_reverse(flag, worker())
}

fn score(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    let first = await helper(true)
    let second = await helper_reverse(false)
    return score(first) + score(second)
}
