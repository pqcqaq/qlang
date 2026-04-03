struct Pair {
    left: Int,
    right: Int,
}

async fn tuple_worker() -> (Bool, Int) {
    return (true, 1)
}

async fn array_worker() -> [Int; 3] {
    return [2, 3, 4]
}

async fn pair_worker() -> Pair {
    return Pair { left: 5, right: 6 }
}

async fn recursive_worker() -> (Pair, [Int; 2]) {
    return (Pair { left: 1, right: 2 }, [3, 4])
}

fn score_tuple(pair: (Bool, Int)) -> Int {
    if pair[0] {
        return pair[1]
    }
    return 0
}

fn score_array(values: [Int; 3]) -> Int {
    return values[0] + values[1] + values[2]
}

fn score_pair(pair: Pair) -> Int {
    return pair.left + pair.right
}

fn score_recursive(result: (Pair, [Int; 2])) -> Int {
    return result[0].left + result[0].right + result[1][0] + result[1][1]
}

async fn main() -> Int {
    let tuple_value = await tuple_worker()
    let array_value = await array_worker()
    let pair_value = await pair_worker()
    let recursive_value = await recursive_worker()

    let tuple_task = spawn tuple_worker()
    let array_task = spawn array_worker()
    let pair_task = spawn pair_worker()
    let recursive_task = spawn recursive_worker()

    let spawned_tuple_value = await tuple_task
    let spawned_array_value = await array_task
    let spawned_pair_value = await pair_task
    let spawned_recursive_value = await recursive_task

    return score_tuple(tuple_value)
        + score_array(array_value)
        + score_pair(pair_value)
        + score_recursive(recursive_value)
        + score_tuple(spawned_tuple_value)
        + score_array(spawned_array_value)
        + score_pair(spawned_pair_value)
        + score_recursive(spawned_recursive_value)
}
