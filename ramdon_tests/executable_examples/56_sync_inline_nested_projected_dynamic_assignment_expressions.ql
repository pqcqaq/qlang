struct Payload {
    values: [Int; 3],
}

struct Env {
    payload: Payload,
}

fn main() -> Int {
    var index = 1
    let first = (Env { payload: Payload { values: [3, 4, 5] } }).payload.values[index] = (Env { payload: Payload { values: [3, 4, 5] } }).payload.values[0] + (Env { payload: Payload { values: [3, 4, 5] } }).payload.values[2]
    return first + (Env { payload: Payload { values: [3, 4, 5] } }).payload.values[index]
}
