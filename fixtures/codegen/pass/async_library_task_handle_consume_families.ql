use tuple_tasks as alias_tuples
use pair_tasks as alias_pairs
use bundle_tasks as alias_bundles
use tuple_env as alias_tuple_env
use pair_env as alias_pair_env
use deep_env as alias_deep_env
use make_tuple_env as alias_make_tuple_env
use make_pair_env as alias_make_pair_env
use make_deep_env as alias_make_deep_env
use worker as run

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

fn tuple_tasks(base: Int) -> (Task[Int], Task[Int]) {
    return (worker(base), worker(base + 1))
}

fn pair_tasks(base: Int) -> Pair {
    return Pair {
        left: worker(base),
        right: worker(base + 1),
    }
}

fn bundle_tasks(base: Int) -> Bundle {
    return Bundle {
        tasks: [worker(base), worker(base + 1)],
    }
}

fn tuple_env(base: Int) -> TupleEnvelope {
    return TupleEnvelope {
        payload: TuplePayload {
            values: (worker(base), worker(base + 1)),
        },
    }
}

fn pair_env(base: Int) -> PairEnvelope {
    return PairEnvelope {
        payload: Pair {
            left: worker(base),
            right: worker(base + 1),
        },
    }
}

fn deep_env(base: Int) -> DeepEnvelope {
    return DeepEnvelope {
        outer: BundleEnvelope {
            payload: Bundle {
                tasks: [worker(base), worker(base + 1)],
            },
        },
    }
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

async fn helper() -> Int {
    var total = 0

    let call_first = await tuple_tasks(10)[0]
    let call_running = spawn pair_tasks(11).left
    let call_third = await bundle_tasks(20).tasks[0]
    let call_last_running = spawn bundle_tasks(0).tasks[1]
    total = total + call_first + call_third + await call_running + await call_last_running

    let alias_first = await alias_tuples(30)[0]
    let alias_running = spawn alias_pairs(31).left
    let alias_third = await alias_bundles(40).tasks[0]
    let alias_last_running = spawn alias_bundles(1).tasks[1]
    total = total + alias_first + alias_third + await alias_running + await alias_last_running

    let nested_first = await tuple_env(50).payload.values[0]
    let nested_running = spawn pair_env(51).payload.left
    let nested_third = await deep_env(60).outer.payload.tasks[0]
    let nested_last_running = spawn deep_env(2).outer.payload.tasks[1]
    total = total + nested_first + nested_third + await nested_running + await nested_last_running

    let alias_nested_first = await alias_tuple_env(70).payload.values[0]
    let alias_nested_running = spawn alias_pair_env(71).payload.left
    let alias_nested_third = await alias_deep_env(80).outer.payload.tasks[0]
    let alias_nested_last_running = spawn alias_deep_env(3).outer.payload.tasks[1]
    total = total + alias_nested_first + alias_nested_third + await alias_nested_running + await alias_nested_last_running

    let awaited_first = await (await make_tuple_env(90)).payload.values[0]
    let awaited_running = spawn (await make_pair_env(91)).payload.left
    let awaited_third = await (await make_deep_env(100)).outer.payload.tasks[0]
    let awaited_last_running = spawn (await make_deep_env(4)).outer.payload.tasks[1]
    total = total + awaited_first + awaited_third + await awaited_running + await awaited_last_running

    let alias_awaited_first = await (await alias_make_tuple_env(110)).payload.values[0]
    let alias_awaited_running = spawn (await alias_make_pair_env(111)).payload.left
    let alias_awaited_third = await (await alias_make_deep_env(120)).outer.payload.tasks[0]
    let alias_awaited_last_running = spawn (await alias_make_deep_env(5)).outer.payload.tasks[1]
    total = total + alias_awaited_first + alias_awaited_third + await alias_awaited_running + await alias_awaited_last_running

    let inline_first = await (TuplePayload {
        values: (worker(130), worker(131)),
    })
        .values[0]
    let inline_running = spawn (Pair {
        left: worker(132),
        right: worker(133),
    })
        .left
    let inline_third = await (DeepEnvelope {
        outer: BundleEnvelope {
            payload: Bundle {
                tasks: [worker(134), worker(135)],
            },
        },
    })
        .outer
        .payload
        .tasks[0]
    let inline_last_running = spawn (DeepEnvelope {
        outer: BundleEnvelope {
            payload: Bundle {
                tasks: [worker(136), worker(137)],
            },
        },
    })
        .outer
        .payload
        .tasks[1]
    total = total + inline_first + inline_third + await inline_running + await inline_last_running

    let alias_inline_first = await (TuplePayload {
        values: (run(140), run(141)),
    })
        .values[0]
    let alias_inline_running = spawn (Pair {
        left: run(142),
        right: run(143),
    })
        .left
    let alias_inline_third = await (DeepEnvelope {
        outer: BundleEnvelope {
            payload: Bundle {
                tasks: [run(144), run(145)],
            },
        },
    })
        .outer
        .payload
        .tasks[0]
    let alias_inline_last_running = spawn (DeepEnvelope {
        outer: BundleEnvelope {
            payload: Bundle {
                tasks: [run(146), run(147)],
            },
        },
    })
        .outer
        .payload
        .tasks[1]
    total = total + alias_inline_first + alias_inline_third + await alias_inline_running + await alias_inline_last_running

    return total
}
