fn add(left: Int, right: Int) -> Int {
    return left + right
}

fn adjust(value: Int) -> Int {
    if value > 10 {
        return value - 1
    }
    return value
}

fn main() -> Int {
    let base = add(20, 23)
    let adjusted = adjust(base)
    if adjusted > 0 {
        return adjusted
    }
    return 0
}
