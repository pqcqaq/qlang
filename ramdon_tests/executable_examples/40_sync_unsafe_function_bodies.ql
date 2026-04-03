unsafe fn seed() -> Int {
    return 2
}

unsafe fn add(left: Int, right: Int) -> Int {
    return left + right
}

fn main() -> Int {
    return add(seed(), 3)
}
