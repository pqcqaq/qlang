struct Boxed {
    values: [Int; 3],
}

extern "c" fn sink(value: Int)

fn helper() -> Boxed {
    return Boxed { values: [1, 2, 3] }
}

fn main() -> Int {
    defer {
        for value in (helper()?).values {
            sink(value)
        }
    }
    return 0
}
