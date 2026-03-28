async fn helper() -> Int {
    var total = 0
    for await value in [1, 2, 3] {
        total = total + value
    }
    return total
}

extern "c" pub fn q_add(left: Int, right: Int) -> Int {
    return left + right
}
