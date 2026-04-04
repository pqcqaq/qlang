struct Wrapper {
    tasks: [Task[Int]; 2],
}

async fn worker(value: Int) -> Int {
    return value
}

async fn helper() -> Int {
    let branch = true
    var wrapper = Wrapper { tasks: [worker(0), worker(0)] }
    var total = 0
    for await value in ({ let current = Wrapper { tasks: [worker(1), worker(2)] }; current }).tasks {
        total = total + value
    }
    for await value in (wrapper = Wrapper { tasks: [worker(3), worker(4)] }).tasks {
        total = total + value
    }
    for await value in (if branch { Wrapper { tasks: [worker(5), worker(6)] } } else { Wrapper { tasks: [worker(7), worker(8)] } }).tasks {
        total = total + value
    }
    for await item in (match branch {
        true => Wrapper { tasks: [worker(9), worker(10)] },
        false => Wrapper { tasks: [worker(11), worker(12)] },
    }).tasks {
        total = total + item
    }
    return total
}
