struct Wrap {
    values: [Int; 0],
}

async fn worker(values: [Int; 0], wrap: Wrap, nested: [[Int; 0]; 1]) -> Int {
    return 7
}

async fn main() -> Int {
    let task = spawn worker([], Wrap { values: [] }, [[]])
    return await task
}
