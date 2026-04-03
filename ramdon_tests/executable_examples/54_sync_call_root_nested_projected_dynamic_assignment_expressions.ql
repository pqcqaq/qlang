struct Payload {
    values: [Int; 3],
}

struct Env {
    payload: Payload,
}

fn make_env() -> Env {
    return Env { payload: Payload { values: [3, 4, 5] } }
}

fn main() -> Int {
    var index = 1
    let first = make_env().payload.values[index] = make_env().payload.values[0] + make_env().payload.values[2]
    return first + make_env().payload.values[index]
}
