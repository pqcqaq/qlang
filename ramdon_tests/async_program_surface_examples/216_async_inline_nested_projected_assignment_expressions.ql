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
    let first = (Env { holder: Holder { pair: Pair { value: 1, values: [2, 3] } } }).holder.pair.value = await worker(4)
    let second = (Env { holder: Holder { pair: Pair { value: 1, values: [2, 3] } } }).holder.pair.values[1] = first + 6
    return first + second
}
