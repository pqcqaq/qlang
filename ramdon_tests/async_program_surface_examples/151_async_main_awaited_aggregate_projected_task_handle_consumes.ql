struct Pair {
    left: Task[Int],
    right: Task[Int],
}

struct TuplePayload {
    values: (Task[Int], Task[Int]),
}

struct TupleEnvelope {
    payload: TuplePayload,
}

struct PairEnvelope {
    payload: Pair,
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

async fn make_tuple_env(base: Int) -> TupleEnvelope {
    return TupleEnvelope {
        payload: TuplePayload {
            values: (worker(base), worker(base + 1)),
        },
    }
}

async fn make_pair_env(base: Int) -> PairEnvelope {
    return PairEnvelope {
        payload: Pair {
            left: worker(base),
            right: worker(base + 1),
        },
    }
}

async fn make_deep_env(base: Int) -> DeepEnvelope {
    return DeepEnvelope {
        outer: BundleEnvelope {
            payload: Bundle {
                tasks: [worker(base), worker(base + 1)],
            },
        },
    }
}

async fn main() -> Int {
    let first = await (await make_tuple_env(10)).payload.values[0]
    let running = spawn (await make_pair_env(11)).payload.left
    let third = await (await make_deep_env(20)).outer.payload.tasks[0]
    let last_running = spawn (await make_deep_env(0)).outer.payload.tasks[1]
    let second = await running
    let last = await last_running
    return first + second + third + last
}
