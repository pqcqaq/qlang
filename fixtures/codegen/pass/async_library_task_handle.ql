async fn worker() -> Int {
    return 1
}

fn schedule() -> Task[Int] {
    return worker()
}

async fn helper() -> Int {
    return await schedule()
}
