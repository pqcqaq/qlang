struct Pair {
    left: Task[Int],
    right: Task[Int],
}

struct TuplePayload {
    values: (Task[Int], Task[Int]),
}

struct Bundle {
    tasks: [Task[Int]; 2],
}

struct BundleEnvelope {
    payload: Bundle,
}

struct DeepEnvelope {
    outer: BundleEnvelope,
}

async fn worker(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    let first = await (TuplePayload {
        values: (worker(10), worker(11)),
    })
        .values[0]
    let running = spawn (Pair {
        left: worker(11),
        right: worker(12),
    })
        .left
    let third = await (DeepEnvelope {
        outer: BundleEnvelope {
            payload: Bundle {
                tasks: [worker(20), worker(21)],
            },
        },
    })
        .outer
        .payload
        .tasks[0]
    let last_running = spawn (DeepEnvelope {
        outer: BundleEnvelope {
            payload: Bundle {
                tasks: [worker(0), worker(1)],
            },
        },
    })
        .outer
        .payload
        .tasks[1]
    let second = await running
    let last = await last_running
    return first + second + third + last
}
