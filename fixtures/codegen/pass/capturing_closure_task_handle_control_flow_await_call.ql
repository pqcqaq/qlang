async fn worker(value: Int) -> Int {
    return value + 1
}

async fn main() -> Int {
    let branch = true
    let which = 1
    let first = spawn worker(41)
    let second = spawn worker(1)
    let left = () => first
    let right = () => second
    let chosen = match which {
        1 => left,
        _ => right,
    }
    let rebound = chosen
    return await (if branch { left } else { right })() + await rebound()
}
