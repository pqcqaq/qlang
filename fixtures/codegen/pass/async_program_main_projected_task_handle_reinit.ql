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
    var tuple = (worker(1), worker(2))
    let tuple_first = await tuple[0]
    tuple[0] = worker(7)
    let tuple_second = await tuple[0]

    var array = [worker(3), worker(4)]
    let array_first = await array[0]
    array[0] = worker(8)
    let array_second = await array[0]

    var pair = TaskPair { left: worker(5), right: worker(6) }
    let struct_first = await pair.left
    pair.left = worker(9)
    let struct_second = await pair.left

    return score(tuple_first)
        + score(tuple_second)
        + score(array_first)
        + score(array_second)
        + score(struct_first)
        + score(struct_second)
}
