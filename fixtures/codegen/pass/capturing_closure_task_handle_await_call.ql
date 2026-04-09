async fn worker(value: Int) -> Int {
    return value + 1
}

async fn main() -> Int {
    let first = spawn worker(41)
    let second = spawn worker(1)
    let direct = () => first
    let fetch = () => second
    let alias = fetch
    return await direct() + await alias()
}
