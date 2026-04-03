struct Pending {
    first: Task[Int],
    second: Task[Int],
}

async fn worker(value: Int) -> Int {
    return value
}

fn schedule(value: Int) -> Task[Int] {
    let task = worker(value)
    return task
}

async fn outer_task(value: Int) -> Task[Int] {
    return worker(value)
}

async fn outer_pending(base: Int) -> Pending {
    return Pending { first: worker(base), second: worker(base + 1) }
}

async fn main() -> Int {
    let first = await schedule(1)

    let second_running = spawn schedule(2)
    let second = await second_running

    let third_task = await outer_task(3)
    let third = await third_task

    let fourth_task = await outer_task(4)
    let fourth_running = spawn fourth_task
    let fourth = await fourth_running

    let pending = await outer_pending(5)
    let fifth = await pending.first
    let sixth_running = spawn pending.second
    let sixth = await sixth_running

    return first + second + third + fourth + fifth + sixth
}
