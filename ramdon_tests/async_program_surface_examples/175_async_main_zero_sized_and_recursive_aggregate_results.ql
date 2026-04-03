struct Wrap {
    values: [Int; 0],
}

struct Pair {
    left: Int,
    right: Int,
}

async fn empty_values() -> [Int; 0] {
    return []
}

async fn wrapped() -> Wrap {
    return Wrap { values: [] }
}

async fn spawned_wrap() -> Wrap {
    return Wrap { values: [] }
}

async fn recursive_worker() -> (Pair, [Int; 2]) {
    return (Pair { left: 1, right: 2 }, [3, 4])
}

fn score_zero(values: [Int; 0], value: Wrap) -> Int {
    return 1
}

fn score_recursive(result: (Pair, [Int; 2])) -> Int {
    return result[0].left + result[0].right + result[1][0] + result[1][1]
}

async fn main() -> Int {
    let zero_values = await empty_values()
    let zero_value = await wrapped()

    let zero_task = spawn spawned_wrap()
    let spawned_zero_value = await zero_task

    let recursive_value = await recursive_worker()

    let recursive_task = spawn recursive_worker()
    let spawned_recursive_value = await recursive_task

    return score_zero(zero_values, zero_value)
        + score_zero([], spawned_zero_value)
        + score_recursive(recursive_value)
        + score_recursive(spawned_recursive_value)
}
