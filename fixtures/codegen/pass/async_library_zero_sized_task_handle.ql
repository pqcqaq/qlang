struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn schedule() -> Task[Wrap] {
    return worker()
}

async fn helper() -> Wrap {
    return await schedule()
}
