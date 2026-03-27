struct Pair {
    left: Int,
    right: Int,
}

async fn worker(pair: Pair, values: [Int; 2]) -> Int {
    return pair.right + values[1]
}

async fn helper() -> Int {
    return await worker(Pair { left: 1, right: 2 }, [3, 4])
}
