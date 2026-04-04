struct TaskArrayPayload {
    tasks: [Task[Int]; 2],
}

struct TaskEnvelope {
    payload: TaskArrayPayload,
}

struct Wrapper {
    tasks: [Task[Int]; 2],
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
        let wrapper = Wrapper { tasks: (await task_env(1)).payload.tasks }
        for await value in wrapper.tasks {
            let copy = value
        }
    }
    return 0
}
