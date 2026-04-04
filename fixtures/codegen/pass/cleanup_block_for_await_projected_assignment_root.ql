struct Wrapper {
    tasks: [Task[Int]; 2],
}

async fn worker(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    var wrapper = Wrapper { tasks: [worker(0), worker(0)] }
    defer {
        for await value in (wrapper = Wrapper { tasks: [worker(1), worker(2)] }).tasks {
            let copy = value
        }
    }
    return 0
}
