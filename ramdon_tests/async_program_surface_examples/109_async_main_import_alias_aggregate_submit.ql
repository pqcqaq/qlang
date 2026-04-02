use worker as run

struct Pending {
    first: Task[Int],
    second: Task[Int],
}

async fn worker(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    let pending = Pending {
        first: run(value: 20),
        second: run(value: 22),
    }
    let first = await pending.first
    let running = spawn pending.second
    let second = await running
    return first + second
}
