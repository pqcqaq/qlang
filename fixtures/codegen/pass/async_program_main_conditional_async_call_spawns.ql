async fn worker() -> Int {
    return 1
}

async fn choose(flag: Bool) -> Int {
    if flag {
        let running = spawn worker();
        return await running
    }
    return await worker()
}

async fn choose_reverse(flag: Bool) -> Int {
    if flag {
        return await worker()
    }
    let running = spawn worker();
    return await running
}

fn score(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    let first = await choose(true)
    let second = await choose_reverse(false)
    return score(first) + score(second)
}
