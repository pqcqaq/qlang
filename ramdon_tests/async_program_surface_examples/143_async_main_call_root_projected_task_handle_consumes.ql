struct Pair {
    left: Task[Int],
    right: Task[Int],
}

struct Bundle {
    tasks: [Task[Int]; 2],
}

async fn worker(value: Int) -> Int {
    return value
}

fn tuple_tasks(base: Int) -> (Task[Int], Task[Int]) {
    return (worker(base), worker(base + 1))
}

fn pair_tasks(base: Int) -> Pair {
    return Pair { left: worker(base), right: worker(base + 1) }
}

fn bundle_tasks(base: Int) -> Bundle {
    return Bundle { tasks: [worker(base), worker(base + 1)] }
}

async fn main() -> Int {
    let first = await tuple_tasks(10)[0]
    let running = spawn pair_tasks(11).left
    let third = await bundle_tasks(20).tasks[0]
    let last_running = spawn bundle_tasks(0).tasks[1]
    let second = await running
    let last = await last_running
    return first + second + third + last
}
