struct Wrap {
    values: [Int; 0],
}

struct TaskPair {
    left: Task[Wrap],
    right: Task[Wrap],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn score(value: Wrap) -> Int {
    return 1
}

async fn main() -> Int {
    var tuple = (worker(), worker())
    let tuple_first = await tuple[0]
    tuple[0] = worker()
    let tuple_second = await tuple[0]

    var array = [worker(), worker()]
    let array_first = await array[0]
    array[0] = worker()
    let array_second = await array[0]

    var pair = TaskPair { left: worker(), right: worker() }
    let struct_first = await pair.left
    pair.left = worker()
    let struct_second = await pair.left

    return score(tuple_first)
        + score(tuple_second)
        + score(array_first)
        + score(array_second)
        + score(struct_first)
        + score(struct_second)
}
