use make_handle as run

fn make_handle(value: Int) -> Task[Int] {
    return worker(value)
}

async fn worker(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    let first = await run(value: 20)
    let running = spawn run(value: 22)
    let second = await running
    return first + second
}
