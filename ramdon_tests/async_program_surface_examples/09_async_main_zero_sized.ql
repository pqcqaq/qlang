struct Wrap {
    values: [Int; 0],
}

struct Slot {
    value: Int,
}

struct TaskPair {
    left: Task[Wrap],
    right: Task[Wrap],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn outer() -> Task[Wrap] {
    return worker()
}

async fn take_zero(values: [Int; 0], wrap: Wrap, nested: [[Int; 0]; 1]) -> Wrap {
    return wrap
}

fn score(value: Wrap) -> Int {
    return 1
}

async fn main() -> Int {
    let zero = await take_zero([], Wrap { values: [] }, [[]])

    let next = await outer()
    let nested = await next

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

    var tasks = [worker(), worker()]
    let slot = Slot { value: 0 }
    let dynamic_first = await tasks[slot.value]
    tasks[slot.value] = worker()
    let dynamic_second = await tasks[slot.value]

    return score(zero)
        + score(nested)
        + score(tuple_first)
        + score(tuple_second)
        + score(array_first)
        + score(array_second)
        + score(struct_first)
        + score(struct_second)
        + score(dynamic_first)
        + score(dynamic_second)
}
