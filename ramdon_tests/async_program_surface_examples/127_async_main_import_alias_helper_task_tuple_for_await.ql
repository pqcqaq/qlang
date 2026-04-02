use make_tasks as schedule

async fn worker(value: Int) -> Int {
    return value
}

fn make_tasks(base: Int) -> (Task[Int], Task[Int]) {
    return (worker(base), worker(base + 2))
}

async fn main() -> Int {
    var total = 0
    for await value in schedule(base: 20) {
        total = total + value
    }
    return total
}
