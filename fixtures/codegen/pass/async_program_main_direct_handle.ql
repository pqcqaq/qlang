async fn worker(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    let first_task = worker(1)
    let second_task = worker(2)
    let first = await first_task
    let second = await second_task
    return first + second
}
