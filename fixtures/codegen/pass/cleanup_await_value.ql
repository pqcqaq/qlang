async fn worker(value: Int) -> Int {
    return value + 1
}

fn forward(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    defer forward(await worker(1))
    return 0
}
