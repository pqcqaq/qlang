struct Wrap {
    values: [Int; 3],
}

async fn worker(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    let seed = await worker(2)
    var index = 1
    var values = [3, 4, 5]
    values[index] = values[0] + values[2] + seed

    var wrap = Wrap { values: [1, 2, 3] }
    wrap.values[index] = values[index] + wrap.values[2]

    return values[1] + wrap.values[1]
}
