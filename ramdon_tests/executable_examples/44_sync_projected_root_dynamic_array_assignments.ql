struct Wrap {
    values: [Int; 3],
}

fn main() -> Int {
    var index = 1
    var wrap = Wrap { values: [3, 4, 5] }
    wrap.values[index] = wrap.values[0] + wrap.values[2]
    return wrap.values[1]
}
