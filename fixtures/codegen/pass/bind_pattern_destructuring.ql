struct Pair {
    left: Int,
    right: Int,
}

fn pair_values() -> [Pair; 2] {
    return [Pair { left: 20, right: 22 }, Pair { left: 24, right: 26 }]
}

fn main() -> Int {
    let (first, second) = (1, 2)
    var total = first + second
    for (left, current) in ((4, 6), (8, 10)) {
        total = total + left + current
    }
    for Pair { left, right: current } in pair_values() {
        total = total + left + current
    }
    return total
}
