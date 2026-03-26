extern "rust" pub fn q_add(left: Int, right: Int) -> Int {
    return left + right
}

fn main() -> Int {
    return q_add(1, 2)
}
