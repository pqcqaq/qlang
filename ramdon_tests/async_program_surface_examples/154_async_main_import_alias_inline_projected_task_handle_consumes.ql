use worker as run

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
        values: (run(10), run(11)),
    })
        .values[0]
    let running = spawn (Pair {
        left: run(11),
        right: run(12),
    })
        .left
    let third = await (DeepEnvelope {
        outer: BundleEnvelope {
            payload: Bundle {
                tasks: [run(20), run(21)],
            },
        },
    })
        .outer
        .payload
        .tasks[0]
    let last_running = spawn (DeepEnvelope {
        outer: BundleEnvelope {
            payload: Bundle {
                tasks: [run(0), run(1)],
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
