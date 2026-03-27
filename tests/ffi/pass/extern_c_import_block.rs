#[no_mangle]
pub extern "C" fn q_host_add(left: i64, right: i64) -> i64 {
    left + right
}

extern "C" {
    fn q_add_two(value: i64) -> i64;
}

fn main() {
    let value = unsafe { q_add_two(40) };
    if value != 42 {
        std::process::exit(1);
    }
}
