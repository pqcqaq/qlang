struct Inner {
    pair: (Int, Int),
}

async fn worker(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    var inner = Inner { pair: (1, 2) }
    let first = inner.pair[0] = await worker(7)
    let second = inner.pair[1] = first + 5
    return inner.pair[0] + second
}
