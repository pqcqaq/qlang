extern "c" pub fn q_export() -> Int {
    return 1
}

async fn worker(value: Int) -> Int {
    return value
}

async fn helper() -> Int {
    var total = 0
    for await value in [worker(20), worker(22)] {
        total = total + value
    }
    return total
}
