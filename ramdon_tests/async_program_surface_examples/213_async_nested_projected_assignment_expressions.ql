struct Pair {
    value: Int,
    values: [Int; 2],
}

struct Holder {
    pair: Pair,
}

struct Env {
    holder: Holder,
}

async fn worker(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    var env = Env { holder: Holder { pair: Pair { value: 1, values: [2, 3] } } }
    let first = env.holder.pair.value = await worker(5)
    let second = env.holder.pair.values[1] = first + 6
    return env.holder.pair.value + env.holder.pair.values[1]
}
