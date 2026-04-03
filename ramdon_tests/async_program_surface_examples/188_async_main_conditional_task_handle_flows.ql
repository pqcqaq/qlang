async fn worker() -> Int {
    return 1
}

async fn fresh_worker() -> Int {
    return 2
}

async fn branch_case(flag: Bool) -> Int {
    var task = worker()
    if flag {
        let running = spawn task
        task = fresh_worker()
        let first = await running
        return first
    } else {
        task = fresh_worker()
    }
    return await task
}

async fn reverse_branch_case(flag: Bool) -> Int {
    var task = worker()
    if flag {
        task = fresh_worker()
    } else {
        let running = spawn task
        task = fresh_worker()
        let first = await running
        return first
    }
    return await task
}

async fn choose(flag: Bool) -> Int {
    if flag {
        let running = spawn worker()
        return await running
    }
    return await worker()
}

async fn choose_reverse(flag: Bool) -> Int {
    if flag {
        return await worker()
    }
    let running = spawn worker()
    return await running
}

async fn choose_task(flag: Bool, task: Task[Int]) -> Int {
    if flag {
        let running = spawn task
        return await running
    }
    return await task
}

async fn choose_task_reverse(flag: Bool, task: Task[Int]) -> Int {
    if flag {
        return await task
    }
    let running = spawn task
    return await running
}

async fn helper(flag: Bool) -> Int {
    return await choose_task(flag, worker())
}

async fn helper_reverse(flag: Bool) -> Int {
    return await choose_task_reverse(flag, worker())
}

fn score(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    let first = await branch_case(true)
    let second = await reverse_branch_case(false)
    let third = await choose(true)
    let fourth = await choose_reverse(false)
    let fifth = await helper(true)
    let sixth = await helper_reverse(false)

    return score(first)
        + score(second)
        + score(third)
        + score(fourth)
        + score(fifth)
        + score(sixth)
}
