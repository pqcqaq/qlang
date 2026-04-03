struct Pending {
    tasks: [Task[Int]; 2],
}

async fn worker(value: Int) -> Int {
    return value
}

fn array_values(base: Int) -> [Int; 2] {
    return [base, base + 1]
}

fn tuple_values(base: Int) -> (Int, Int) {
    return (base, base + 1)
}

fn task_values(base: Int) -> (Task[Int], Task[Int]) {
    return (worker(base), worker(base + 1))
}

fn make_pending(base: Int) -> Pending {
    return Pending {
        tasks: [worker(base), worker(base + 1)],
    }
}

async fn main() -> Int {
    var total = 0
    for await value in array_values(8) {
        total = total + value
    }
    for await value in tuple_values(4) {
        total = total + value
    }
    for await value in task_values(2) {
        total = total + value
    }
    for await value in make_pending(5).tasks {
        total = total + value
    }
    return total
}
