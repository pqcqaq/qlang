async fn main() -> Int {
    var total = 0
    for await value in (20, 22) {
        total = total + value
    }
    return total
}
