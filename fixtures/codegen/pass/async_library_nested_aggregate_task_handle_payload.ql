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

async fn outer() -> [Pending; 2] {
    return [
        Pending { task: left(), value: 10 },
        Pending { task: right(), value: 20 },
    ]
}

async fn helper() -> Int {
    let pending = await outer()
    let first = await pending[0].task
    let second = await pending[1].task
    return first + second + pending[0].value + pending[1].value
}
