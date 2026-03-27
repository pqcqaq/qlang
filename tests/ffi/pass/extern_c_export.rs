extern "C" {
    fn q_add(left: i64, right: i64) -> i64;
}

fn main() {
    let value = unsafe { q_add(20, 22) };
    if value != 42 {
        std::process::exit(1);
    }
}
