async fn worker(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    let first_task = worker(1)
    let second_task = worker(2)
    let first_running = spawn first_task
    let second_running = spawn second_task
    let first = await first_running
    let second = await second_running
    return first + second
}
