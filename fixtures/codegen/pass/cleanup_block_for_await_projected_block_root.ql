struct Wrapper {
    tasks: [Task[Int]; 2],
}

async fn worker(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    defer {
        for await value in ({ let wrapper = Wrapper { tasks: [worker(1), worker(2)] }; wrapper }).tasks {
            let copy = value
        }
    }
    return 0
}
