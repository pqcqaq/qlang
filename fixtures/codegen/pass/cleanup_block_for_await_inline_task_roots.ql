extern "c" fn step(value: Int)

async fn worker(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    defer {
        for await value in [worker(1), worker(2)] {
            step(value);
        }
        for await item in (worker(3), worker(4)) {
            step(item);
        }
    }
    return 0
}
