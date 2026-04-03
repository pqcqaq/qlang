struct Pending {
    task: Task[Int],
    value: Int,
}

struct Wrap {
    values: [Int; 0],
}

async fn left() -> Int {
    return 1
}

async fn right() -> Int {
    return 2
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn array_outer() -> [Task[Int]; 2] {
    return [left(), right()]
}

async fn tuple_outer() -> (Task[Int], Task[Int]) {
    return (left(), right())
}

async fn pending_outer() -> [Pending; 2] {
    return [
        Pending { task: left(), value: 10 },
        Pending { task: right(), value: 20 },
    ]
}

async fn nested_outer() -> Task[Wrap] {
    return worker()
}

async fn helper() -> Int {
    let tasks = await array_outer()
    let first = await tasks[0]
    let second = await tasks[1]

    let pair = await tuple_outer()
    let third = await pair[0]
    let fourth = await pair[1]

    let pending = await pending_outer()
    let fifth = await pending[0].task
    let sixth = await pending[1].task

    let next = await nested_outer()
    let wrap = await next

    return first
        + second
        + third
        + fourth
        + fifth
        + sixth
        + pending[0].value
        + pending[1].value
}

extern "c" pub fn q_export() -> Int {
    return 1
}
