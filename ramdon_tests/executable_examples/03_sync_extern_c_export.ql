extern "c" pub fn q_scale(left: Int, right: Int) -> Int {
    return left * right
}

fn main() -> Int {
    return q_scale(6, 7)
}
