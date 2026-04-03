async fn worker(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    var total = 1
    let first = total = await worker(7)
    let second = total = first + 5
    return total + first + second
}
