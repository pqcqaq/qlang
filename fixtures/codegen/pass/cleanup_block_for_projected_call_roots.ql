struct Holder {
    values: [Int; 3],
}

extern "c" fn sink(value: Int)

fn items() -> [Int; 3] {
    return [4, 5, 6]
}

fn main() -> Int {
    let holder = Holder { values: [1, 2, 3] }
    defer {
        for value in holder.values {
            sink(value)
        }
        for item in items() {
            sink(item)
        }
    }
    return 0
}
