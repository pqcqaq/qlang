struct TaskPair {
    left: Task[Int],
    right: Task[Int],
}

async fn worker(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    var tuple = (worker(1), worker(2))
    let tuple_first = await tuple[0]
    let tuple_running = spawn tuple[1]
    let tuple_spawned = await tuple_running
    tuple[0] = worker(7)
    let tuple_second = await tuple[0]

    var array = [worker(3), worker(4)]
    let array_first = await array[0]
    let array_running = spawn array[1]
    let array_spawned = await array_running
    array[0] = worker(8)
    let array_second = await array[0]

    var pair = TaskPair { left: worker(5), right: worker(6) }
    let struct_first = await pair.left
    let struct_running = spawn pair.right
    let struct_spawned = await struct_running
    pair.left = worker(9)
    let struct_second = await pair.left

    return tuple_first
        + tuple_spawned
        + tuple_second
        + array_first
        + array_spawned
        + array_second
        + struct_first
        + struct_spawned
        + struct_second
}
