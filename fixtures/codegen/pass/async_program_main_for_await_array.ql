async fn main() -> Int {
    var total = 0
    for await value in [1, 2, 3] {
        total = total + value
    }
    return total
}
