/// Rust host callbacks provided to Qlang.
///
/// These are declared `#[no_mangle] extern "C"` so that the Qlang staticlib
/// can resolve them at link time via its `extern "c" fn` import declarations.

#[no_mangle]
pub extern "C" fn q_host_add(left: i64, right: i64) -> i64 {
    left + right
}

#[no_mangle]
pub extern "C" fn q_host_multiply(left: i64, right: i64) -> i64 {
    left * right
}

/// Qlang exports linked from the staticlib produced by `ql build --emit staticlib`.
unsafe extern "C" {
    fn q_add_two(value: i64) -> i64;
    fn q_scale(value: i64, factor: i64) -> i64;
}

fn main() {
    // Verify q_add_two: Rust → Qlang → Rust (q_host_add) → Qlang → Rust
    let add_result = unsafe { q_add_two(40) };
    assert_eq!(
        add_result, 42,
        "expected q_add_two(40) == 42 (Qlang calls q_host_add(40, 2))"
    );

    // Verify q_scale: Rust → Qlang → Rust (q_host_multiply) → Qlang → Rust
    let scale_result = unsafe { q_scale(6, 7) };
    assert_eq!(
        scale_result, 42,
        "expected q_scale(6, 7) == 42 (Qlang calls q_host_multiply(6, 7))"
    );

    println!("q_add_two(40) = {add_result}");
    println!("q_scale(6, 7) = {scale_result}");
}
