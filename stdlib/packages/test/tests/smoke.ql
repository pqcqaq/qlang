use std.core.abs_int as abs_int
use std.core.bool_to_int as bool_to_int
use std.core.clamp_int as clamp_int
use std.core.in_range_int as in_range_int
use std.core.is_even_int as is_even_int
use std.core.is_odd_int as is_odd_int
use std.core.max_int as max_int
use std.core.min_int as min_int
use std.core.sign_int as sign_int

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
    let sign_negative = expect_int_eq(sign_int(0 - 7), 0 - 1)
    let sign_zero = expect_int_eq(sign_int(0), 0)
    let sign_positive = expect_int_eq(sign_int(7), 1)

    let even_result = bool_to_int(is_even_int(8))
    let odd_result = bool_to_int(is_odd_int(9))
    let range_mid = bool_to_int(in_range_int(5, 3, 9))
    let range_low = bool_to_int(in_range_int(2, 3, 9))

    return max_result + min_result + clamp_low + clamp_mid + clamp_high + abs_result + bool_result + sign_negative + sign_zero + sign_positive + expect_int_eq(even_result, 1) + expect_int_eq(odd_result, 1) + expect_int_eq(range_mid, 1) + expect_int_eq(range_low, 0)
}
