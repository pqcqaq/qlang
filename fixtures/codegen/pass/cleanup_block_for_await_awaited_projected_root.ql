struct TaskArrayPayload {
    tasks: [Task[Int]; 2],
}

struct TaskEnvelope {
    payload: TaskArrayPayload,
}

async fn worker(value: Int) -> Int {
    return value
}

async fn task_env(base: Int) -> TaskEnvelope {
    return TaskEnvelope {
        payload: TaskArrayPayload {
            tasks: [worker(base), worker(base + 1)],
        },
    }
}

async fn main() -> Int {
    defer {
        for await value in (await task_env(1)).payload.tasks {
            let copy = value
        }
    }
    return 0
}
