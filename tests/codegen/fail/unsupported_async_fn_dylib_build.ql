extern "c" pub fn q_export() -> Int {
    return 1
}

fn worker() -> Int {
    return 1
}

async fn helper() -> Int {
    return await worker()
}
