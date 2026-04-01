struct Pair {
    left: Int,
    right: Int,
}

async fn tuple_worker() -> (Bool, Int) {
    return (true, 2)
}

async fn array_worker() -> [Int; 3] {
    return [3, 4, 5]
}

async fn pair_worker() -> Pair {
    return Pair { left: 6, right: 7 }
}

async fn combine(pair: Pair, values: [Int; 2]) -> Int {
    return pair.left + pair.right + values[0] + values[1]
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
    var total = 0
    for await value in [1, 2, 3] {
        total = total + value
    }

    let tuple_value = await tuple_worker()
    let array_value = await array_worker()
    let pair_value = await pair_worker()
    let combined = await combine(Pair { left: 8, right: 9 }, [10, 11])

    return total
        + score_tuple(tuple_value)
        + score_array(array_value)
        + score_pair(pair_value)
        + combined
}
