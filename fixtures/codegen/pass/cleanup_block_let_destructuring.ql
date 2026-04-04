struct Pair {
    left: Int,
    right: Int,
}

extern "c" fn sink(value: Int)

fn values() -> (Int, Int) {
    return (4, 6)
}

fn pair_value() -> Pair {
    return Pair { left: 20, right: 22 }
}

fn main() -> Int {
    defer {
        let (first, _) = values();
        let Pair { left, right: current } = pair_value();
        sink(first);
        sink(left);
        sink(current)
    }
    return 0
}
