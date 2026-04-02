use worker as run
use score as total

async fn worker(value: Int) -> Int {
    return value
}

fn score(left: Int, right: Int) -> Int {
    return left + right
}

async fn main() -> Int {
    let task = run(value: 20)
    let first = await task
    return total(right: 22, left: first)
}
