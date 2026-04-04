struct Wrapper {
    tasks: [Task[Int]; 2],
}

async fn worker(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    let branch = true
    defer {
        for await value in (if branch { Wrapper { tasks: [worker(1), worker(2)] } } else { Wrapper { tasks: [worker(3), worker(4)] } }).tasks {
            let copy = value
        }
        for await item in (match branch { true => Wrapper { tasks: [worker(5), worker(6)] }, false => Wrapper { tasks: [worker(7), worker(8)] } }).tasks {
            let copy = item
        }
    }
    return 0
}
