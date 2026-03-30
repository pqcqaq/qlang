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
    let tuple_running = spawn tuple[0]
    let tuple_value = await tuple_running

    let array = [worker(), worker()]
    let array_running = spawn array[0]
    let array_value = await array_running

    let pair = TaskPair { left: worker(), right: worker() }
    let struct_running = spawn pair.left
    let struct_value = await struct_running

    return score(tuple_value) + score(array_value) + score(struct_value)
}
