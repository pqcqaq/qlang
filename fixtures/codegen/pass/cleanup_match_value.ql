fn forward(value: Int) -> Int {
    return value
}

fn main() -> Int {
    var value = 0
    defer {
        forward(match value {
            0 => 1,
            _ => 2,
        })
    }
    return 0
}
