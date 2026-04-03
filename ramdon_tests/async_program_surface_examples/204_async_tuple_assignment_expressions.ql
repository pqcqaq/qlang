async fn worker(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    var pair = (1, 2)
    let first = pair[0] = await worker(7)
    let second = pair[1] = first + 5
    return pair[0] + second
}
