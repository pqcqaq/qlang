use std.core.abs_int as abs_int
use std.core.and_bool as and_bool
use std.core.bool_to_int as bool_to_int
use std.core.clamp_int as clamp_int
use std.core.compare_int as compare_int
use std.core.implies_bool as implies_bool
use std.core.in_exclusive_range_int as in_exclusive_range_int
use std.core.in_range_int as in_range_int
use std.core.is_divisible_by_int as is_divisible_by_int
use std.core.is_even_int as is_even_int
use std.core.is_negative_int as is_negative_int
use std.core.is_nonzero_int as is_nonzero_int
use std.core.is_odd_int as is_odd_int
use std.core.is_positive_int as is_positive_int
use std.core.is_zero_int as is_zero_int
use std.core.max_int as max_int
use std.core.min_int as min_int
use std.core.not_bool as not_bool
use std.core.or_bool as or_bool
use std.core.sign_int as sign_int
use std.core.xor_bool as xor_bool

fn expect_int_eq(actual: Int, expected: Int) -> Int {
    if actual == expected {
        return 0
    }
    return 1
}

fn expect_bool_eq(actual: Bool, expected: Bool) -> Int {
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
    let compare_less = expect_int_eq(compare_int(3, 9), 0 - 1)
    let compare_equal = expect_int_eq(compare_int(9, 9), 0)
    let compare_greater = expect_int_eq(compare_int(9, 3), 1)

    let even_result = bool_to_int(is_even_int(8))
    let odd_result = bool_to_int(is_odd_int(9))
    let range_mid = bool_to_int(in_range_int(5, 3, 9))
    let range_low = bool_to_int(in_range_int(2, 3, 9))
    let exclusive_mid = expect_bool_eq(in_exclusive_range_int(5, 3, 9), true)
    let exclusive_edge = expect_bool_eq(in_exclusive_range_int(3, 3, 9), false)
    let zero_result = expect_bool_eq(is_zero_int(0), true)
    let nonzero_result = expect_bool_eq(is_nonzero_int(5), true)
    let positive_result = expect_bool_eq(is_positive_int(5), true)
    let negative_result = expect_bool_eq(is_negative_int(0 - 5), true)
    let divisible_result = expect_bool_eq(is_divisible_by_int(21, 7), true)
    let zero_divisor_result = expect_bool_eq(is_divisible_by_int(21, 0), false)
    let not_result = expect_bool_eq(not_bool(false), true)
    let and_result = expect_bool_eq(and_bool(true, false), false)
    let or_result = expect_bool_eq(or_bool(false, true), true)
    let xor_result = expect_bool_eq(xor_bool(true, false), true)
    let implies_result = expect_bool_eq(implies_bool(true, false), false)

    return max_result + min_result + clamp_low + clamp_mid + clamp_high + abs_result + bool_result + sign_negative + sign_zero + sign_positive + compare_less + compare_equal + compare_greater + expect_int_eq(even_result, 1) + expect_int_eq(odd_result, 1) + expect_int_eq(range_mid, 1) + expect_int_eq(range_low, 0) + exclusive_mid + exclusive_edge + zero_result + nonzero_result + positive_result + negative_result + divisible_result + zero_divisor_result + not_result + and_result + or_result + xor_result + implies_result
}
