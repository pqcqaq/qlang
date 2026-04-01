struct Pending {
    tasks: [Task[Int]; 2],
    tail: Task[Int],
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

fn forward(task: Task[Int]) -> Task[Int] {
    return task
}

async fn main() -> Int {
    let row = choose()
    let slots = [row, row]
    let alias_slots = slots
    var pending = Pending {
        tasks: [worker(9), worker(14)],
        tail: worker(17),
    }
    let alias = pending.tasks
    let first = await alias[alias_slots[row]]
    pending.tasks[slots[row]] = worker(first + 9)
    let env = Envelope {
        bundle: Bundle {
            tasks: [forward(alias[alias_slots[row]]), worker(24)],
        },
        tail: pending.tail,
    }
    let running = spawn env.bundle.tasks[0]
    let second = await running
    let extra = await env.bundle.tasks[1]
    let tail = await env.tail
    return second + extra + tail
}
