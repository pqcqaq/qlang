use worker as run

async fn worker(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    let pending = (run(value: 20), run(value: 22))
    let first = await pending[0]
    let running = spawn pending[1]
    let second = await running
    return first + second
}
