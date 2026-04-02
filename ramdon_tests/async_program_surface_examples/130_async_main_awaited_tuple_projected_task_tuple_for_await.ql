async fn worker(value: Int) -> Int {
    return value
}

async fn make_pair(base: Int) -> ((Task[Int], Task[Int]), Int) {
    return ((worker(base), worker(base + 2)), 0)
}

async fn main() -> Int {
    let pair = await make_pair(20)
    var total = 0
    for await value in pair[0] {
        total = total + value
    }
    return total
}
