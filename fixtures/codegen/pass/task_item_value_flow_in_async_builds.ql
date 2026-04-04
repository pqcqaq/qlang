use MORE as STATIC_ENV
use BOX as CONST_ENV

struct Pending {
    tasks: [Task[Int]; 2],
}

extern "c" fn step(value: Int)

const BOX: Pending = Pending { tasks: [worker(1), worker(2)] }
static MORE: Pending = Pending { tasks: [worker(3), worker(4)] }

fn forward(env: Pending) -> Pending {
    return env
}

async fn worker(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    let branch = true
    let pending = forward(if branch { BOX } else { STATIC_ENV })
    let matched = forward(match branch {
        true => CONST_ENV,
        false => MORE,
    })
    var total = 0
    for await value in pending.tasks {
        total = total + value
    }
    total = total + await matched.tasks[0]
    defer {
        let cleanup_pending = forward(if branch { BOX } else { STATIC_ENV })
        for await value in cleanup_pending.tasks {
            step(value);
        }
        let cleanup_matched = forward(match branch {
            true => CONST_ENV,
            false => MORE,
        })
        step(await cleanup_matched.tasks[0]);
    }
    return total
}
