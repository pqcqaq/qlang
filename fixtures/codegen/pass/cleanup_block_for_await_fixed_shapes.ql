extern "c" fn step(value: Int)
extern "c" fn finish(value: Int)

async fn worker(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    let tasks = (worker(3), worker(4))
    defer {
        for await value in [1, 2] {
            step(value);
            continue;
            finish(value);
        }
        for await item in tasks {
            step(item);
            break;
            finish(item);
        }
    }
    return 0
}
