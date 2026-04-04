extern "c" fn step(value: Int)

async fn worker(value: Int) -> Int {
    return value
}

fn task_array() -> [Task[Int]; 2] {
    return [worker(1), worker(2)]
}

fn task_pair() -> (Task[Int], Task[Int]) {
    return (worker(3), worker(4))
}

async fn main() -> Int {
    defer {
        for await value in task_array()? {
            step(value);
        }
        for await item in task_pair()? {
            step(item);
        }
        for await tail in ({ let tasks = [worker(5), worker(6)]; tasks })? {
            step(tail);
        }
    }
    return 0
}
