use items as load_items

struct Holder {
    values: [Int; 3],
}

extern "c" fn sink(value: Int)

fn items() -> [Int; 3] {
    return [4, 5, 6]
}

fn holder() -> Holder {
    return Holder { values: [1, 2, 3] }
}

fn main() -> Int {
    defer {
        for value in holder().values {
            sink(value)
        }
        for item in load_items() {
            sink(item)
        }
    }
    return 0
}
