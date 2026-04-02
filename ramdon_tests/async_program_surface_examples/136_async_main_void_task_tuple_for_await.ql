async fn ping() -> Void {
    return
}

async fn main() -> Int {
    var total = 0
    for await _ in (ping(), ping()) {
        total = total + 1
    }
    return total
}
