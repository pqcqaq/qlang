extern "c" fn q_host_add(left: Int, right: Int) -> Int

extern "c" pub fn q_add_two(value: Int) -> Int {
    return q_host_add(value, 2)
}
