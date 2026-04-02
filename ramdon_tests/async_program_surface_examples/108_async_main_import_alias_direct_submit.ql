use worker as run

async fn worker(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    let direct = await run(value: 20)
    let running = spawn run(value: 22)
    let spawned = await running
    return direct + spawned
}
