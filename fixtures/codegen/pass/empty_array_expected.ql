fn values() -> [Int; 0] {
    return []
}

fn pair() -> ([Int; 0], Int) {
    return ([], 1)
}

struct Wrap {
    values: [Int; 0],
}

fn build() -> Wrap {
    return Wrap { values: [] }
}

fn nested() -> [[Int; 0]; 1] {
    return [[]]
}

fn take(values: [Int; 0]) -> Int {
    return 0
}

fn main() -> Int {
    let pair_value = pair()
    let wrap = build()
    let nested_values = nested()
    return take(values()) + pair_value[1] + take(wrap.values) + take(nested_values[0]) + take([])
}
