extern "c" fn step(value: Int)

async fn worker(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    let branch = true
    defer if branch {
        let values = [worker(1), worker(2)]
        for await value in values {
            step(value);
        }
    } else {
        step(0);
    }
    defer match branch {
        true => {
            let values = [worker(3), worker(4)]
            for await value in values {
                step(value);
            }
        }
        false => {
            step(0);
        }
    }
    return 0
}
