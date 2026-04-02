use collect as run

fn collect(values: [Int; 0], left: Int, right: Int) -> Int {
    return left + right + 7
}

fn main() -> Int {
    return run(right: 20, values: [], left: 22)
}
