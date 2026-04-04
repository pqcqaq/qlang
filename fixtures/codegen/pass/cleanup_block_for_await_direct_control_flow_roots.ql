extern "c" fn step(value: Int)

async fn worker(value: Int) -> Int {
    return value
}

async fn load_values(base: Int) -> [Int; 2] {
    return [base, base + 1]
}

async fn load_tasks(base: Int) -> [Task[Int]; 2] {
    return [worker(base), worker(base + 1)]
}

async fn load_task_pair(base: Int) -> (Task[Int], Task[Int]) {
    return (worker(base), worker(base + 1))
}

async fn main() -> Int {
    let branch = true
    var tasks = [worker(0), worker(0)]
    defer {
        for await value in ({ let current = [worker(1), worker(2)]; current }) {
            step(value);
        }
        for await value in (tasks = [worker(3), worker(4)]) {
            step(value);
        }
        for await value in (if branch { [worker(5), worker(6)] } else { [worker(7), worker(8)] }) {
            step(value);
        }
        for await item in (match branch {
            true => [worker(9), worker(10)],
            false => [worker(11), worker(12)],
        }) {
            step(item);
        }
        for await scalar in await load_values(13) {
            step(scalar);
        }
        for await awaited in await load_tasks(15) {
            step(awaited);
        }
        for await pair in await load_task_pair(17) {
            step(pair);
        }
    }
    return 0
}
