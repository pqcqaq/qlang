struct Payload {
    values: [Int; 3],
}

struct Env {
    payload: Payload,
}

async fn worker(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    let seed = await worker(2)
    var index = 1
    var env = Env { payload: Payload { values: [3, 4, 5] } }
    let first = env.payload.values[index] = env.payload.values[0] + env.payload.values[2] + seed
    return first + env.payload.values[index]
}
