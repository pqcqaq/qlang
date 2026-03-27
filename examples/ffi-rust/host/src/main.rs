#[no_mangle]
pub extern "C" fn q_host_add(left: i64, right: i64) -> i64 {
    left + right
}

unsafe extern "C" {
    fn q_add_two(value: i64) -> i64;
}

fn main() {
    let value = unsafe { q_add_two(40) };
    assert_eq!(value, 42, "expected Rust callback + Qlang export roundtrip");
    println!("q_add_two(40) = {value}");
}
