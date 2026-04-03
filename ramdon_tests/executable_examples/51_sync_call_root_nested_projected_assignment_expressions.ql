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

fn make_env() -> Env {
    return Env { holder: Holder { pair: Pair { value: 1, values: [2, 3] } } }
}

fn main() -> Int {
    let first = make_env().holder.pair.value = 4
    let second = make_env().holder.pair.values[1] = first + 6
    return first + second
}
