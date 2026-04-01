async fn worker(value: Int) -> Int {
    return value
}

fn make_handle(value: Int) -> Task[Int] {
    return worker(value)
}

fn local_return_handle(value: Int) -> Task[Int] {
    let task = worker(value)
    return task
}

fn forward(task: Task[Int]) -> Task[Int] {
    return task
}

async fn main() -> Int {
    let direct = await worker(1)

    let bound = worker(2)
    let bound_value = await bound

    let running = spawn worker(3)
    let spawned = await running

    let helper = make_handle(4)
    let helper_value = await helper

    let local_returned = local_return_handle(5)
    let local_returned_value = await local_returned

    let forwarded_source = make_handle(6)
    let forwarded = forward(forwarded_source)
    let forwarded_value = await forwarded

    let spawned_source = make_handle(7)
    let spawned_running = spawn forward(spawned_source)
    let spawned_forwarded_value = await spawned_running

    return direct
        + bound_value
        + spawned
        + helper_value
        + local_returned_value
        + forwarded_value
        + spawned_forwarded_value
}
