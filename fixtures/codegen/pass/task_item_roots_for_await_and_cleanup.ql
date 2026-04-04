use MORE as ITEMS
use TASK_PAIR as PAIRS

extern "c" fn step(value: Int)

const TASKS: [Task[Int]; 2] = [worker(1), worker(2)]
static MORE: [Task[Int]; 2] = [worker(3), worker(4)]
const TASK_PAIR: (Task[Int], Task[Int]) = (worker(5), worker(6))

async fn worker(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    var total = 0
    for await value in TASKS {
        total = total + value
    }
    for await item in ITEMS {
        total = total + item
    }
    for await pair in PAIRS {
        total = total + pair
    }
    defer {
        for await value in TASKS {
            step(value);
        }
        for await item in ITEMS {
            step(item);
        }
        for await pair in PAIRS {
            step(pair);
        }
    }
    return total
}
