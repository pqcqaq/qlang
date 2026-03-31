struct TaskPair {
    left: Task[Int],
    right: Task[Int],
}

async fn worker(value: Int) -> Int {
    return value
}

fn score(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    let tuple = (worker(1), worker(2))
    let tuple_running = spawn tuple[0]
    let tuple_value = await tuple_running

    let array = [worker(3), worker(4)]
    let array_running = spawn array[0]
    let array_value = await array_running

    let pair = TaskPair { left: worker(5), right: worker(6) }
    let struct_running = spawn pair.left
    let struct_value = await struct_running

    return score(tuple_value) + score(array_value) + score(struct_value)
}
