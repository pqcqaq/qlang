struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn choose(flag: Bool) -> Wrap {
    if flag {
        let running = spawn worker();
        return await running
    }
    return await worker()
}

async fn choose_reverse(flag: Bool) -> Wrap {
    if flag {
        return await worker()
    }
    let running = spawn worker();
    return await running
}

fn score(value: Wrap) -> Int {
    return 1
}

async fn main() -> Int {
    let first = await choose(true)
    let second = await choose_reverse(false)
    return score(first) + score(second)
}
