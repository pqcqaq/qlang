fn add_one(value: Int) -> Int {
    return value + 1
}

fn main() -> Int {
    let value = add_one(41)
    if value > 0 {
        return value
    }
    return 0
}
