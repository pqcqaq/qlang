struct Bundle {
    tasks: (Task[Int], Task[Int]),
}

struct Env {
    bundle: Bundle,
}

async fn worker(value: Int) -> Int {
    return value
}

async fn make_env(base: Int) -> Env {
    return Env { bundle: Bundle { tasks: (worker(base), worker(base + 2)) } }
}

async fn main() -> Int {
    let env = await make_env(20)
    var total = 0
    for await value in env.bundle.tasks {
        total = total + value
    }
    return total
}
