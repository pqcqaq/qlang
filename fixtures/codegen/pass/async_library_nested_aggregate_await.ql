struct Pair {
    left: Int,
    right: Int,
}

async fn worker() -> (Pair, [Int; 2]) {
    return (Pair { left: 1, right: 2 }, [3, 4])
}

async fn helper() -> (Pair, [Int; 2]) {
    return await worker()
}
