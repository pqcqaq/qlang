struct Payload {
    values: [Int; 3],
}

struct Env {
    payload: Payload,
}

fn make_env() -> Env {
    return Env { payload: Payload { values: [3, 4, 5] } }
}

async fn worker(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    let seed = await worker(2)
    var index = 1
    let first = make_env().payload.values[index] = make_env().payload.values[0] + make_env().payload.values[2] + seed
    return first + make_env().payload.values[index]
}
