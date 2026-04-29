use std.test.expect_bool_and as expect_bool_and
use std.test.expect_bool_eq as expect_bool_eq
use std.test.expect_bool_implies as expect_bool_implies
use std.test.expect_bool_ne as expect_bool_ne
use std.test.expect_bool_not as expect_bool_not
use std.test.expect_bool_or as expect_bool_or
use std.test.expect_bool_xor as expect_bool_xor
use std.test.expect_false as expect_false
use std.test.expect_int_average2 as expect_int_average2
use std.test.expect_int_average3 as expect_int_average3
use std.test.expect_int_ascending as expect_int_ascending
use std.test.expect_int_between as expect_int_between
use std.test.expect_int_between_bounds as expect_int_between_bounds
use std.test.expect_int_clamped as expect_int_clamped
use std.test.expect_int_clamped_bounds as expect_int_clamped_bounds
use std.test.expect_int_descending as expect_int_descending
use std.test.expect_int_distance_to_bounds as expect_int_distance_to_bounds
use std.test.expect_int_distance_to_range as expect_int_distance_to_range
use std.test.expect_int_divisible_by as expect_int_divisible_by
use std.test.expect_int_eq as expect_int_eq
use std.test.expect_int_even as expect_int_even
use std.test.expect_int_exclusive_between as expect_int_exclusive_between
use std.test.expect_int_exclusive_between_bounds as expect_int_exclusive_between_bounds
use std.test.expect_int_ge as expect_int_ge
use std.test.expect_int_gt as expect_int_gt
use std.test.expect_int_le as expect_int_le
use std.test.expect_int_lt as expect_int_lt
use std.test.expect_int_max as expect_int_max
use std.test.expect_int_max3 as expect_int_max3
use std.test.expect_int_max4 as expect_int_max4
use std.test.expect_int_min as expect_int_min
use std.test.expect_int_min3 as expect_int_min3
use std.test.expect_int_min4 as expect_int_min4
use std.test.expect_int_ne as expect_int_ne
use std.test.expect_int_negative as expect_int_negative
use std.test.expect_int_nonnegative as expect_int_nonnegative
use std.test.expect_int_nonpositive as expect_int_nonpositive
use std.test.expect_int_not_within as expect_int_not_within
use std.test.expect_int_odd as expect_int_odd
use std.test.expect_int_outside as expect_int_outside
use std.test.expect_int_outside_bounds as expect_int_outside_bounds
use std.test.expect_int_positive as expect_int_positive
use std.test.expect_int_product3 as expect_int_product3
use std.test.expect_int_product4 as expect_int_product4
use std.test.expect_int_strictly_descending as expect_int_strictly_descending
use std.test.expect_int_strictly_ascending as expect_int_strictly_ascending
use std.test.expect_int_sum3 as expect_int_sum3
use std.test.expect_int_sum4 as expect_int_sum4
use std.test.expect_int_within as expect_int_within
use std.test.expect_nonzero as expect_nonzero
use std.test.expect_status_failed as expect_status_failed
use std.test.expect_status_ok as expect_status_ok
use std.test.expect_true as expect_true
use std.test.expect_zero as expect_zero
use std.test.is_status_failed as is_status_failed
use std.test.is_status_ok as is_status_ok
use std.test.merge_status as merge_status
use std.test.merge_status3 as merge_status3
use std.test.merge_status4 as merge_status4
use std.test.merge_status5 as merge_status5
use std.test.merge_status6 as merge_status6

fn check_int(actual: Int, expected: Int) -> Int {
    if actual == expected {
        return 0
    }
    return 1
}

fn check_bool(actual: Bool, expected: Bool) -> Int {
    if actual == expected {
        return 0
    }
    return 1
}

fn sum4(first: Int, second: Int, third: Int, fourth: Int) -> Int {
    return first + second + third + fourth
}

fn main() -> Int {
    let bool_pass = sum4(check_int(expect_true(true), 0), check_int(expect_false(false), 0), check_int(expect_bool_eq(true, true), 0), check_int(expect_bool_ne(true, false), 0))
    let bool_logic_pass = sum4(check_int(expect_bool_not(false, true), 0), check_int(expect_bool_and(true, false, false), 0), check_int(expect_bool_or(false, true, true), 0), check_int(expect_bool_xor(true, true, false), 0))
    let bool_failure = sum4(check_int(expect_true(false), 1), check_int(expect_false(true), 1), check_int(expect_bool_eq(true, false), 1), check_int(expect_bool_ne(true, true), 1))
    let bool_logic_failure = sum4(check_int(expect_bool_not(false, false), 1), check_int(expect_bool_and(true, false, true), 1), check_int(expect_bool_or(false, false, true), 1), check_int(expect_bool_xor(true, false, false), 1))

    let int_order_pass = sum4(check_int(expect_int_eq(8, 8), 0), check_int(expect_int_ne(8, 9), 0), check_int(expect_int_gt(9, 8), 0), check_int(expect_int_ge(8, 8), 0))
    let int_boundary_pass = sum4(check_int(expect_int_lt(7, 8), 0), check_int(expect_int_le(8, 8), 0), check_int(expect_zero(0), 0), check_int(expect_nonzero(1), 0))
    let int_order_failure = sum4(check_int(expect_int_eq(8, 9), 1), check_int(expect_int_ne(8, 8), 1), check_int(expect_int_gt(8, 8), 1), check_int(expect_int_ge(7, 8), 1))
    let int_boundary_failure = sum4(check_int(expect_int_lt(8, 8), 1), check_int(expect_int_le(9, 8), 1), check_int(expect_zero(1), 1), check_int(expect_nonzero(0), 1))

    let range_pass = sum4(check_int(expect_int_between(5, 3, 9), 0), check_int(expect_int_exclusive_between(5, 3, 9), 0), check_int(expect_int_outside(2, 3, 9), 0), check_int(expect_int_between_bounds(5, 9, 3), 0))
    let bounds_pass = sum4(check_int(expect_int_exclusive_between_bounds(5, 9, 3), 0), check_int(expect_int_outside_bounds(2, 9, 3), 0), check_int(expect_int_ascending(3, 3, 9), 0), check_int(expect_int_descending(9, 9, 3), 0))
    let order_pass = sum4(check_int(expect_int_strictly_ascending(3, 5, 9), 0), check_int(expect_int_strictly_descending(9, 5, 3), 0), 0, 0)
    let range_failure = sum4(check_int(expect_int_between(2, 3, 9), 1), check_int(expect_int_exclusive_between(3, 3, 9), 1), check_int(expect_int_outside(5, 3, 9), 1), check_int(expect_int_between_bounds(10, 9, 3), 1))
    let bounds_failure = sum4(check_int(expect_int_exclusive_between_bounds(9, 9, 3), 1), check_int(expect_int_outside_bounds(5, 9, 3), 1), check_int(expect_int_ascending(9, 5, 3), 1), check_int(expect_int_descending(3, 9, 5), 1))
    let order_failure = sum4(check_int(expect_int_strictly_ascending(3, 3, 9), 1), check_int(expect_int_strictly_descending(9, 9, 3), 1), 0, 0)
    let transform_pass = sum4(check_int(expect_int_clamped(12, 3, 9, 9), 0), check_int(expect_int_clamped_bounds(2, 9, 3, 3), 0), check_int(expect_int_distance_to_range(2, 3, 9, 1), 0), check_int(expect_int_distance_to_bounds(10, 9, 3, 1), 0))
    let transform_failure = sum4(check_int(expect_int_clamped(12, 3, 9, 12), 1), check_int(expect_int_clamped_bounds(2, 9, 3, 2), 1), check_int(expect_int_distance_to_range(5, 3, 9, 1), 1), check_int(expect_int_distance_to_bounds(5, 9, 3, 1), 1))
    let aggregate_pass = sum4(check_int(expect_int_sum3(2, 3, 4, 9), 0), check_int(expect_int_sum4(2, 3, 4, 5, 14), 0), check_int(expect_int_product3(2, 3, 4, 24), 0), check_int(expect_int_product4(2, 3, 4, 5, 120), 0))
    let average_pass = sum4(check_int(expect_int_average2(5, 8, 6), 0), check_int(expect_int_average3(3, 6, 9, 6), 0), 0, 0)
    let aggregate_failure = sum4(check_int(expect_int_sum3(2, 3, 4, 10), 1), check_int(expect_int_sum4(2, 3, 4, 5, 15), 1), check_int(expect_int_product3(2, 3, 4, 25), 1), check_int(expect_int_product4(2, 3, 4, 5, 121), 1))
    let average_failure = sum4(check_int(expect_int_average2(5, 8, 7), 1), check_int(expect_int_average3(3, 6, 9, 7), 1), 0, 0)
    let extrema_pass = sum4(check_int(expect_int_max(20, 22, 22), 0), check_int(expect_int_min(20, 22, 20), 0), check_int(expect_int_max3(20, 22, 21, 22), 0), check_int(expect_int_min3(20, 22, 21, 20), 0))
    let extrema4_pass = sum4(check_int(expect_int_max4(20, 22, 21, 19, 22), 0), check_int(expect_int_min4(20, 22, 21, 19, 19), 0), 0, 0)
    let extrema_failure = sum4(check_int(expect_int_max(20, 22, 20), 1), check_int(expect_int_min(20, 22, 22), 1), check_int(expect_int_max3(20, 22, 21, 21), 1), check_int(expect_int_min3(20, 22, 21, 21), 1))
    let extrema4_failure = sum4(check_int(expect_int_max4(20, 22, 21, 19, 21), 1), check_int(expect_int_min4(20, 22, 21, 19, 20), 1), 0, 0)

    let number_pass = sum4(check_int(expect_int_even(8), 0), check_int(expect_int_odd(9), 0), check_int(expect_int_divisible_by(21, 7), 0), check_int(expect_int_within(11, 10, 1), 0))
    let sign_pass = sum4(check_int(expect_int_not_within(12, 10, 1), 0), check_int(expect_int_positive(1), 0), check_int(expect_int_negative(0 - 1), 0), check_int(expect_int_nonnegative(0), 0))
    let number_failure = sum4(check_int(expect_int_even(9), 1), check_int(expect_int_odd(8), 1), check_int(expect_int_divisible_by(21, 0), 1), check_int(expect_int_within(12, 10, 1), 1))
    let sign_failure = sum4(check_int(expect_int_not_within(10, 10, 0), 1), check_int(expect_int_positive(0), 1), check_int(expect_int_negative(0), 1), check_int(expect_int_nonnegative(0 - 1), 1))

    let status_bool = sum4(check_bool(is_status_ok(0), true), check_bool(is_status_ok(1), false), check_bool(is_status_failed(1), true), check_bool(is_status_failed(0), false))
    let status_merge = sum4(check_int(merge_status(1, 2), 3), check_int(merge_status3(1, 2, 3), 6), check_int(merge_status4(1, 2, 3, 4), 10), check_int(merge_status5(1, 2, 3, 4, 5), 15))
    let status_merge_large = sum4(check_int(merge_status6(1, 2, 3, 4, 5, 6), 21), check_int(expect_bool_implies(false, false), 0), 0, 0)
    let status_expect = sum4(check_int(expect_status_ok(0), 0), check_int(expect_status_ok(1), 1), check_int(expect_status_failed(1), 0), check_int(expect_status_failed(0), 1))
    let sign_boundary = sum4(check_int(expect_int_nonpositive(0), 0), check_int(expect_int_nonpositive(1), 1), check_int(expect_bool_implies(true, false), 1), 0)

    let bool_status = sum4(bool_pass, bool_logic_pass, bool_failure, bool_logic_failure)
    let int_status = sum4(int_order_pass, int_boundary_pass, int_order_failure, int_boundary_failure)
    let range_status = sum4(range_pass, bounds_pass, range_failure, bounds_failure)
    let order_status = sum4(order_pass, order_failure, 0, 0)
    let number_status = sum4(number_pass, sign_pass, number_failure, sign_failure)
    let status_status = sum4(status_bool, status_merge, status_merge_large, status_expect)

    return check_int(sum4(bool_status, int_status, range_status, sum4(order_status, number_status, status_status, sum4(sign_boundary, transform_pass, transform_failure, sum4(aggregate_pass + extrema_pass, average_pass + extrema4_pass, aggregate_failure + extrema_failure, average_failure + extrema4_failure)))), 0)
}
