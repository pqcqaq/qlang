extern "c" fn q_host_log(message: *const U8) -> Void

extern "c" {
    fn q_host_add(left: Int, right: Int) -> Int
}

extern "c" pub fn q_exported(value: Int) -> Int {
    return value
}
