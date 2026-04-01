struct Pending {
    tasks: [Task[Int]; 2],
}

struct Bundle {
    tasks: [Task[Int]; 2],
}

struct Envelope {
    bundle: Bundle,
    tail: Task[Int],
}

async fn worker(value: Int) -> Int {
    return value
}

fn choose() -> Int {
    return 0
}

async fn main() -> Int {
    let row = choose()
    let slots = [row, row]
    var pending = Pending {
        tasks: [worker(9), worker(14)],
    }
    let alias = pending.tasks
    let first = await alias[slots[row]]
    pending.tasks[slots[row]] = worker(first + 6)
    let env = Envelope {
        bundle: Bundle {
            tasks: [alias[slots[row]], worker(18)],
        },
        tail: pending.tasks[1],
    }
    let running = spawn env.bundle.tasks[0]
    let second = await running
    let extra = await env.bundle.tasks[1]
    let tail = await env.tail
    return second + extra + tail
}
