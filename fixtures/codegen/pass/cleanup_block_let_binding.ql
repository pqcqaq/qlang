extern "c" fn sink(value: Int)

fn amount() -> Int {
    return 41
}

fn main() -> Int {
    defer {
        let value = amount()
        sink(value)
    }
    return 0
}
