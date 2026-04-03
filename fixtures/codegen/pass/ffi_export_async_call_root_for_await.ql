async fn worker(value: Int) -> Int {
    return value
}

fn array_values(base: Int) -> [Int; 2] {
    return [base, base + 1]
}

fn tuple_values(base: Int) -> (Int, Int) {
    return (base, base + 1)
}

fn task_array_values(base: Int) -> [Task[Int]; 2] {
    return [worker(base), worker(base + 1)]
}

fn task_tuple_values(base: Int) -> (Task[Int], Task[Int]) {
    return (worker(base), worker(base + 1))
}

async fn helper() -> Int {
    var total = 0
    for await value in array_values(1) {
        total = total + value
    }
    for await value in tuple_values(3) {
        total = total + value
    }
    for await value in task_array_values(5) {
        total = total + value
    }
    for await value in task_tuple_values(7) {
        total = total + value
    }
    return total
}

extern "c" pub fn q_export() -> Int {
    return 1
}
