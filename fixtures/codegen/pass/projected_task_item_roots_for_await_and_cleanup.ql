use MORE as STATIC_ENV
use BOX as CONST_ENV

struct Pending {
    tasks: [Task[Int]; 2],
}

extern "c" fn step(value: Int)

const BOX: Pending = Pending { tasks: [worker(1), worker(2)] }
static MORE: Pending = Pending { tasks: [worker(3), worker(4)] }

async fn worker(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    var total = 0
    for await value in BOX.tasks {
        total = total + value
    }
    for await value in STATIC_ENV.tasks {
        total = total + value
    }
    for await value in CONST_ENV.tasks {
        total = total + value
    }
    defer {
        for await value in BOX.tasks {
            step(value);
        }
        for await value in STATIC_ENV.tasks {
            step(value);
        }
        for await value in CONST_ENV.tasks {
            step(value);
        }
    }
    return total
}
