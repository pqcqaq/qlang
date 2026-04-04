fn forward(value: Int) -> Int {
    return value
}

fn main() -> Int {
    var value = 0
    defer {
        forward(if value == 0 { 1 } else { 2 })
    }
    return 0
}
