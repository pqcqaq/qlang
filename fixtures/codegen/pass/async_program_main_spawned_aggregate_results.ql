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

async fn main() -> Int {
    let tuple_task = spawn tuple_worker()
    let array_task = spawn array_worker()
    let pair_task = spawn pair_worker()
    let tuple_value = await tuple_task
    let array_value = await array_task
    let pair_value = await pair_task
    return score_tuple(tuple_value) + score_array(array_value) + score_pair(pair_value)
}
