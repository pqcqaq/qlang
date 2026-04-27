use std.core.abs_int as abs_int
use std.core.bool_to_int as bool_to_int
use std.core.clamp_int as clamp_int
use std.core.max_int as max_int
use std.core.min_int as min_int

fn expect_int_eq(actual: Int, expected: Int) -> Int {
    if actual == expected {
        return 0
    }
    return 1
}

fn main() -> Int {
    let max_result = expect_int_eq(max_int(3, 9), 9)
    let min_result = expect_int_eq(min_int(3, 9), 3)
    let clamp_low = expect_int_eq(clamp_int(1, 3, 9), 3)
    let clamp_mid = expect_int_eq(clamp_int(5, 3, 9), 5)
    let clamp_high = expect_int_eq(clamp_int(12, 3, 9), 9)
    let abs_result = expect_int_eq(abs_int(0 - 7), 7)
    let bool_result = expect_int_eq(bool_to_int(true), 1)

    return max_result + min_result + clamp_low + clamp_mid + clamp_high + abs_result + bool_result
}
