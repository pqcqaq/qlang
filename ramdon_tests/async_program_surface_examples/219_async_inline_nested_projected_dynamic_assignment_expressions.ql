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
    let first = (Env { payload: Payload { values: [3, 4, 5] } }).payload.values[index] = (Env { payload: Payload { values: [3, 4, 5] } }).payload.values[0] + (Env { payload: Payload { values: [3, 4, 5] } }).payload.values[2] + seed
    return first + (Env { payload: Payload { values: [3, 4, 5] } }).payload.values[index]
}
