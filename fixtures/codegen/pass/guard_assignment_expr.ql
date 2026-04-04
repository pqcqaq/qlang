fn forward(value: Int) -> Int {
    return value
}

fn main() -> Int {
    var cleanup_enabled = false
    defer if cleanup_enabled = true { forward(1) } else { forward(0) }
    return 0
}
