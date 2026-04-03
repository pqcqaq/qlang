struct Wrap {
    values: [Int; 3],
}

fn main() -> Int {
    var index = 1
    var values = [3, 4, 5]
    let first = values[index] = values[0] + values[2]
    var wrap = Wrap { values: [1, 2, 3] }
    let second = wrap.values[index] = first + wrap.values[2]
    return first + second + values[index] + wrap.values[index]
}
