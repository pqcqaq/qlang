async fn seed_total() -> Int {
    var total = 0
    for await value in [20, 22] {
        total = total + value
    }
    return total
}

async fn helper() -> Int {
    return await seed_total()
}

extern "c" pub fn q_add(left: Int, right: Int) -> Int {
    return left + right
}
