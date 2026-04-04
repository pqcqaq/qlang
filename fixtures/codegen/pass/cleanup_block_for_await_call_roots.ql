use tasks as load_tasks

struct TaskArrayPayload {
    tasks: [Task[Int]; 2],
}

struct TaskEnvelope {
    payload: TaskArrayPayload,
}

extern "c" fn step(value: Int)

async fn worker(value: Int) -> Int {
    return value
}

fn tasks(base: Int) -> [Task[Int]; 2] {
    return [worker(base), worker(base + 1)]
}

fn task_env(base: Int) -> TaskEnvelope {
    return TaskEnvelope {
        payload: TaskArrayPayload {
            tasks: [worker(base), worker(base + 1)],
        },
    }
}

async fn main() -> Int {
    defer {
        for await value in tasks(1) {
            step(value);
        }
        for await item in load_tasks(3) {
            step(item);
        }
        for await tail in task_env(5).payload.tasks {
            step(tail);
        }
    }
    return 0
}
