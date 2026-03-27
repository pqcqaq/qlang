struct Pair {
    left: Int,
    right: Int,
}

struct Outer {
    pair: Pair,
    values: [Int; 2],
}

fn pick_pair(outer: Outer) -> Int {
    return outer.pair.right
}

fn pick_array(outer: Outer, index: Int) -> Int {
    return outer.values[index]
}

fn main() -> Int {
    let outer = Outer {
        pair: Pair { left: 1, right: 2 },
        values: [3, 4],
    }
    return pick_pair(outer) + pick_array(outer, 1)
}
