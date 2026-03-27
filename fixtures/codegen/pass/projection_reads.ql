struct Pair {
    left: Bool,
    right: Int,
}

fn pick_struct(pair: Pair) -> Int {
    return pair.right
}

fn pick_tuple(pair: (Bool, Int)) -> Int {
    return pair[1]
}

fn pick_array(values: [Int; 3], index: Int) -> Int {
    return values[index]
}

fn main() -> Int {
    let pair = Pair { left: false, right: 7 }
    let tuple = (true, 5)
    let values = [1, 2, 3]
    return pick_struct(pair) + pick_tuple(tuple) + pick_array(values, 1)
}
