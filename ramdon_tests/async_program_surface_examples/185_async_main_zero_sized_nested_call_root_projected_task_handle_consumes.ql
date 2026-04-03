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

struct TupleEnvelope {
    payload: TuplePayload,
}

struct PairEnvelope {
    payload: Pair,
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

fn tuple_env() -> TupleEnvelope {
    return TupleEnvelope {
        payload: TuplePayload {
            values: (worker(), worker()),
        },
    }
}

fn pair_env() -> PairEnvelope {
    return PairEnvelope {
        payload: Pair {
            left: worker(),
            right: worker(),
        },
    }
}

fn deep_env() -> DeepEnvelope {
    return DeepEnvelope {
        outer: BundleEnvelope {
            payload: Bundle {
                tasks: [worker(), worker()],
            },
        },
    }
}

fn score(value: Wrap) -> Int {
    return 1
}

async fn main() -> Int {
    let first = await tuple_env().payload.values[0]
    let running = spawn pair_env().payload.left
    let third = await deep_env().outer.payload.tasks[0]
    let last_running = spawn deep_env().outer.payload.tasks[1]
    let second = await running
    let last = await last_running
    return score(first) + score(second) + score(third) + score(last)
}
