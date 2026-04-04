use worker as run_alias

async fn worker(value: Int) -> Int {
    return value + 1
}

async fn main() -> Int {
    let direct = worker
    let aliased = run_alias
    let first = await direct(10)
    let second = await aliased(20)
    return first + second
}
