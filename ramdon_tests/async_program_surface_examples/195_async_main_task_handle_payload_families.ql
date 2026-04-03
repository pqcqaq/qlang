struct Pending {
    task: Task[Int],
    value: Int,
}

async fn worker(value: Int) -> Int {
    return value
}

async fn outer_task(value: Int) -> Task[Int] {
    return worker(value)
}

async fn tuple_payload(base: Int) -> (Task[Int], Task[Int]) {
    return (worker(base), worker(base + 1))
}

async fn array_payload(base: Int) -> [Task[Int]; 2] {
    return [worker(base), worker(base + 1)]
}

async fn nested_payload(base: Int) -> [Pending; 2] {
    return [
        Pending { task: worker(base), value: base + 10 },
        Pending { task: worker(base + 1), value: base + 11 },
    ]
}

async fn main() -> Int {
    let next = await outer_task(1)
    let first = await next

    let pair = await tuple_payload(2)
    let second = await pair[0]
    let pair_running = spawn pair[1]
    let third = await pair_running

    let tasks = await array_payload(4)
    let fourth = await tasks[0]
    let array_running = spawn tasks[1]
    let fifth = await array_running

    let pending = await nested_payload(6)
    let sixth = await pending[0].task
    let nested_running = spawn pending[1].task
    let seventh = await nested_running

    return first
        + second
        + third
        + fourth
        + fifth
        + sixth
        + seventh
        + pending[0].value
        + pending[1].value
}
