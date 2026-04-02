use make_handle as run

fn make_handle(value: Int) -> Task[Int] {
    return worker(value)
}

fn forward(task: Task[Int]) -> Task[Int] {
    return task
}

async fn worker(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    let first = await forward(run(value: 20))
    let running = spawn forward(run(value: 22))
    let second = await running
    return first + second
}
