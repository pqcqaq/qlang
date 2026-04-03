struct Wrap {
    values: [Int; 0],
}

struct Pair {
    left: Task[Wrap],
    right: Task[Wrap],
}

struct TuplePayload {
    values: (Task[Wrap], Task[Wrap]),
}

struct Bundle {
    tasks: [Task[Wrap]; 2],
}

struct BundleEnvelope {
    payload: Bundle,
}

struct DeepEnvelope {
    outer: BundleEnvelope,
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn score(value: Wrap) -> Int {
    return 1
}

async fn main() -> Int {
    let first = await (TuplePayload {
        values: (worker(), worker()),
    })
        .values[0]
    let running = spawn (Pair {
        left: worker(),
        right: worker(),
    })
        .left
    let third = await (DeepEnvelope {
        outer: BundleEnvelope {
            payload: Bundle {
                tasks: [worker(), worker()],
            },
        },
    })
        .outer
        .payload
        .tasks[0]
    let last_running = spawn (DeepEnvelope {
        outer: BundleEnvelope {
            payload: Bundle {
                tasks: [worker(), worker()],
            },
        },
    })
        .outer
        .payload
        .tasks[1]
    let second = await running
    let last = await last_running
    return score(first) + score(second) + score(third) + score(last)
}
