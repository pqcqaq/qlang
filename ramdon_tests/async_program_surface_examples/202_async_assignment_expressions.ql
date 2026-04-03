struct Pair {
    value: Int,
    values: [Int; 2],
}

async fn worker(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    var pair = Pair { value: 1, values: [2, 3] }
    let seed = await worker(1)
    let first = pair.value = seed + 3
    let second = pair.values[1] = first + 5
    return seed + first + second + pair.value + pair.values[1]
}
