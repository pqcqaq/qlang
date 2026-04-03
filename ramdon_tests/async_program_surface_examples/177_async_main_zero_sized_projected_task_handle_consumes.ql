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
    let tuple = (worker(), worker())
    let tuple_first = await tuple[0]
    let tuple_running = spawn tuple[1]
    let tuple_second = await tuple_running

    let array = [worker(), worker()]
    let array_first = await array[0]
    let array_running = spawn array[1]
    let array_second = await array_running

    let pair = TaskPair { left: worker(), right: worker() }
    let struct_first = await pair.left
    let struct_running = spawn pair.right
    let struct_second = await struct_running

    return score(tuple_first)
        + score(tuple_second)
        + score(array_first)
        + score(array_second)
        + score(struct_first)
        + score(struct_second)
}
