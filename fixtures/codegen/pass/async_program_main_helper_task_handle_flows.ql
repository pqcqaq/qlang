async fn worker() -> Int {
    return 1
}

async fn other() -> Int {
    return 2
}

fn schedule() -> Task[Int] {
    return worker()
}

fn forward(task: Task[Int]) -> Task[Int] {
    return task
}

async fn main() -> Int {
    let direct = await schedule()

    let bound = schedule()
    let bound_value = await bound

    let spawned = spawn schedule()
    let spawned_value = await spawned

    let task = other()
    let forwarded = forward(task)
    let forwarded_value = await forwarded

    let next = worker()
    let running = spawn forward(next)
    let running_value = await running

    return direct + bound_value + spawned_value + forwarded_value + running_value
}
