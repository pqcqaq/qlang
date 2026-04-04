struct Wrapper {
    tasks: [Task[Int]; 2],
}

async fn worker(value: Int) -> Int {
    return value
}

fn helper() -> Wrapper {
    return Wrapper { tasks: [worker(1), worker(2)] }
}

async fn main() -> Int {
    defer {
        for await value in (helper()?).tasks {
            let copy = value
        }
    }
    return 0
}
