struct Pair {
    value: Int,
    values: [Int; 2],
}

struct Holder {
    pair: Pair,
}

async fn worker(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    var holder = Holder { pair: Pair { value: 1, values: [2, 3] } }
    let first = holder.pair.value = await worker(4)
    let second = holder.pair.values[1] = first + 5
    return holder.pair.value + holder.pair.values[1]
}
