struct Pair {
    left: Int,
    right: Int,
}

extern "c" fn sink(value: Int)

fn pair_values() -> [Pair; 2] {
    return [Pair { left: 20, right: 22 }, Pair { left: 24, right: 26 }]
}

fn main() -> Int {
    defer {
        for (first, _) in ((4, 6), (8, 10)) {
            sink(first);
        }
        for Pair { left, right: current } in pair_values() {
            sink(left);
            sink(current);
        }
    }
    return 0
}
