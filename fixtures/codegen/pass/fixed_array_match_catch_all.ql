extern "c" fn sink(value: Int)

fn main() -> Int {
    match [1, 2, 3] {
        [first, _, last] if first < last => sink(first + last),
        _ => sink(0),
    }

    defer match [4, 5, 6] {
        [first, middle, last] if middle == 5 => sink(first + middle + last),
        _ => sink(0),
    }

    return 0
}
