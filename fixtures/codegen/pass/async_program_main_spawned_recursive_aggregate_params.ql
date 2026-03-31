struct Pair {
    left: Int,
    right: Int,
}

async fn worker(pair: Pair, values: [Int; 2]) -> Int {
    return pair.right + values[1]
}

async fn main() -> Int {
    let task = spawn worker(Pair { left: 1, right: 2 }, [3, 4])
    return await task
}
