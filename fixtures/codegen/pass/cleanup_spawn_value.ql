async fn worker(value: Int) -> Int {
    return value + 1
}

fn keep(task: Task[Int]) -> Int {
    return 0
}

async fn main() -> Int {
    defer keep(spawn worker(1))
    return 0
}
