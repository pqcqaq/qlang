use tuple_tasks as tuples
use pair_tasks as pairs
use bundle_tasks as bundles
use make_tuple_env as tuple_env
use make_pair_env as pair_env
use make_deep_env as deep_env

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

fn tuple_tasks() -> (Task[Wrap], Task[Wrap]) {
    return (worker(), worker())
}

fn pair_tasks() -> Pair {
    return Pair { left: worker(), right: worker() }
}

fn bundle_tasks() -> Bundle {
    return Bundle { tasks: [worker(), worker()] }
}

async fn make_tuple_env() -> TupleEnvelope {
    return TupleEnvelope {
        payload: TuplePayload {
            values: (worker(), worker()),
        },
    }
}

async fn make_pair_env() -> PairEnvelope {
    return PairEnvelope {
        payload: Pair {
            left: worker(),
            right: worker(),
        },
    }
}

async fn make_deep_env() -> DeepEnvelope {
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
    let first = await tuples()[0]
    let running = spawn pairs().left
    let third = await bundles().tasks[0]

    let fourth = await (await tuple_env()).payload.values[0]
    let awaited_running = spawn (await pair_env()).payload.left
    let sixth = await (await deep_env()).outer.payload.tasks[0]

    let second = await running
    let fifth = await awaited_running

    return score(first)
        + score(second)
        + score(third)
        + score(fourth)
        + score(fifth)
        + score(sixth)
}
