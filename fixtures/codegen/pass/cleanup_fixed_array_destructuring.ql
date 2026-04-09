extern "c" fn sink(value: Int)

async fn worker(value: Int) -> [Int; 2] {
    return [value, value + 1]
}

async fn main() -> Int {
    defer {
        let [first, _, third] = [1, 2, 3]
        sink(first + third)

        for [left, right] in ([4, 5], [6, 7]) {
            sink(left + right)
        }

        for await [left, right] in [worker(8), worker(10)] {
            sink(left + right)
        }
    }

    return 0
}
