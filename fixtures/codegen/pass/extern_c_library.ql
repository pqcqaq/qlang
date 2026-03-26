extern "c" {
    fn q_add(left: Int, right: Int) -> Int
}

fn add_two(value: Int) -> Int {
    return q_add(value, 2)
}
