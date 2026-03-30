struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn outer() -> Task[Wrap] {
    return worker()
}

fn score(value: Wrap) -> Int {
    return 1
}

async fn main() -> Int {
    let next = await outer()
    let value = await next
    return score(value)
}
