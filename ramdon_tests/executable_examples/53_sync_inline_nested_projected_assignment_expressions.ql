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

fn main() -> Int {
    let first = (Env { holder: Holder { pair: Pair { value: 1, values: [2, 3] } } }).holder.pair.value = 4
    let second = (Env { holder: Holder { pair: Pair { value: 1, values: [2, 3] } } }).holder.pair.values[1] = first + 6
    return first + second
}
