struct Pending {
    task: Task[Int],
    value: Int,
}

async fn left() -> Int {
    return 1
}

async fn right() -> Int {
    return 2
}

async fn tuple_outer() -> (Task[Int], Task[Int]) {
    return (left(), right())
}

async fn array_outer() -> [Task[Int]; 2] {
    return [left(), right()]
}

async fn nested_outer() -> [Pending; 2] {
    return [
        Pending { task: left(), value: 10 },
        Pending { task: right(), value: 20 },
    ]
}

async fn main() -> Int {
    let pair = await tuple_outer()
    let first = await pair[0]
    let second = await pair[1]

    let tasks = await array_outer()
    let third = await tasks[0]
    let fourth = await tasks[1]

    let pending = await nested_outer()
    let fifth = await pending[0].task
    let sixth = await pending[1].task

    return first
        + second
        + third
        + fourth
        + fifth
        + sixth
        + pending[0].value
        + pending[1].value
}
