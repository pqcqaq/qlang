async fn worker(value: Int) -> Int {
    return value
}

fn schedule(value: Int) -> Task[Int] {
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
    let helper_direct = await schedule(1)

    let helper_bound = schedule(2)
    let helper_bound_value = await helper_bound

    let helper_spawned = spawn schedule(3)
    let helper_spawned_value = await helper_spawned

    let helper_bound_task = schedule(4)
    let helper_bound_running = spawn helper_bound_task
    let helper_bound_spawned_value = await helper_bound_running

    let local_returned = local_return_handle(5)
    let local_returned_value = await local_returned

    let forwarded_source = worker(6)
    let forwarded = forward(forwarded_source)
    let forwarded_value = await forwarded

    let spawned_source = worker(7)
    let spawned_running = spawn forward(spawned_source)
    let spawned_forwarded_value = await spawned_running

    return helper_direct
        + helper_bound_value
        + helper_spawned_value
        + helper_bound_spawned_value
        + local_returned_value
        + forwarded_value
        + spawned_forwarded_value
}
