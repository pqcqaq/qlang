struct Inner {
    pair: (Int, Int),
}

fn main() -> Int {
    var inner = Inner { pair: (1, 2) }
    let first = inner.pair[0] = 7
    let second = inner.pair[1] = first + 5
    return inner.pair[0] + second
}
