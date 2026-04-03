struct Wrap {
    values: [Int; 0],
}

async fn int_worker(value: Int) -> Int {
    return value
}

async fn wrap_worker() -> Wrap {
    return Wrap { values: [] }
}

fn int_schedule(base: Int) -> Task[Int] {
    return int_worker(base)
}

fn int_schedule_local(base: Int) -> Task[Int] {
    let task = int_worker(base)
    return task
}

fn int_forward(task: Task[Int]) -> Task[Int] {
    return task
}

fn wrap_schedule() -> Task[Wrap] {
    return wrap_worker()
}

fn wrap_schedule_local() -> Task[Wrap] {
    let task = wrap_worker()
    return task
}

fn wrap_forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn scalar_flows() -> Int {
    let direct = await int_worker(1)
    let direct_bound = await {
        let task = int_worker(2)
        task
    }
    let scheduled = await int_schedule(3)
    let bound = await {
        let task = int_schedule(4)
        task
    }
    let local = await int_schedule_local(5)
    let forwarded = await {
        let task = int_worker(6)
        let forwarded = int_forward(task)
        forwarded
    }
    let spawned_direct = await spawn int_worker(7)
    let spawned_bound_direct = await {
        let task = int_worker(8)
        let running = spawn task
        running
    }
    let spawned_schedule = await spawn int_schedule(9)
    let spawned_bound = await {
        let task = int_schedule(10)
        let running = spawn task
        running
    }
    let spawned_forward = await {
        let task = int_worker(11)
        let running = spawn int_forward(task)
        running
    }
    return direct
        + direct_bound
        + scheduled
        + bound
        + local
        + forwarded
        + spawned_direct
        + spawned_bound_direct
        + spawned_schedule
        + spawned_bound
        + spawned_forward
}

async fn wrap_flows() -> Wrap {
    let direct = await wrap_worker()
    let direct_bound = await {
        let task = wrap_worker()
        task
    }
    let scheduled = await wrap_schedule()
    let bound = await {
        let task = wrap_schedule()
        task
    }
    let local = await wrap_schedule_local()
    let forwarded = await {
        let task = wrap_worker()
        let forwarded = wrap_forward(task)
        forwarded
    }
    let spawned_direct = await spawn wrap_worker()
    let spawned_schedule = await spawn wrap_schedule()
    let spawned_bound_direct = await {
        let task = wrap_worker()
        let running = spawn task
        running
    }
    let spawned_bound = await {
        let task = wrap_schedule()
        let running = spawn task
        running
    }
    let spawned_forward = await {
        let task = wrap_worker()
        let running = spawn wrap_forward(task)
        running
    }
    return spawned_forward
}

async fn helper() -> Int {
    let total = await scalar_flows()
    let final_wrap = await wrap_flows()
    return total
}
