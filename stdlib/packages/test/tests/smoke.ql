use std.core.median_ints as median_ints
use std.test.expect_bool_and as expect_bool_and
use std.test.expect_bool_array_all as expect_bool_array_all
use std.test.expect_bool_array_at as expect_bool_array_at
use std.test.expect_bool_array_any as expect_bool_array_any
use std.test.expect_bool_array_contains as expect_bool_array_contains
use std.test.expect_bool_array_count as expect_bool_array_count
use std.test.expect_bool_array_first as expect_bool_array_first
use std.test.expect_bool_array_last as expect_bool_array_last
use std.test.expect_bool_array_none as expect_bool_array_none
use std.test.expect_bool_array_reverse as expect_bool_array_reverse
use std.test.expect_bool_eq as expect_bool_eq
use std.test.expect_bool_implies as expect_bool_implies
use std.test.expect_bool_ne as expect_bool_ne
use std.test.expect_bool_not as expect_bool_not
use std.test.expect_bool_or as expect_bool_or
use std.test.expect_bool_to_int as expect_bool_to_int
use std.test.expect_bool_xor as expect_bool_xor
use std.test.expect_bool_option_none as expect_bool_option_none
use std.test.expect_bool_option_ok_or as expect_bool_option_ok_or
use std.test.expect_bool_option_ok_or_err as expect_bool_option_ok_or_err
use std.test.expect_bool_option_or as expect_bool_option_or
use std.test.expect_bool_option_some as expect_bool_option_some
use std.test.expect_bool_result_err as expect_bool_result_err
use std.test.expect_bool_result_error_none as expect_bool_result_error_none
use std.test.expect_bool_result_error_some as expect_bool_result_error_some
use std.test.expect_bool_result_ok as expect_bool_result_ok
use std.test.expect_bool_result_or as expect_bool_result_or
use std.test.expect_bool_result_to_option_none as expect_bool_result_to_option_none
use std.test.expect_bool_result_to_option_some as expect_bool_result_to_option_some
use std.test.expect_array_at as expect_array_at
use std.test.expect_array_contains as expect_array_contains
use std.test.expect_array_count as expect_array_count
use std.test.expect_array_eq as expect_array_eq
use std.test.expect_array_first as expect_array_first
use std.test.expect_array_last as expect_array_last
use std.test.expect_array_reverse as expect_array_reverse
use std.test.expect_eq as expect_eq
use std.test.expect_false as expect_false
use std.test.expect_int_abs as expect_int_abs
use std.test.expect_int_abs_diff as expect_int_abs_diff
use std.test.expect_int_array_at as expect_int_array_at
use std.test.expect_int_array_average as expect_int_array_average
use std.test.expect_int_array_contains as expect_int_array_contains
use std.test.expect_int_array_count as expect_int_array_count
use std.test.expect_int_array_descending as expect_int_array_descending
use std.test.expect_int_array_first as expect_int_array_first
use std.test.expect_int_array_last as expect_int_array_last
use std.test.expect_int_array_max as expect_int_array_max
use std.test.expect_int_array_min as expect_int_array_min
use std.test.expect_int_array_product as expect_int_array_product
use std.test.expect_int_array_reverse as expect_int_array_reverse
use std.test.expect_int_array_ascending as expect_int_array_ascending
use std.test.expect_int_array_strictly_ascending as expect_int_array_strictly_ascending
use std.test.expect_int_array_strictly_descending as expect_int_array_strictly_descending
use std.test.expect_int_array_sum as expect_int_array_sum
use std.test.expect_int_between as expect_int_between
use std.test.expect_int_between_bounds as expect_int_between_bounds
use std.test.expect_int_clamp_max as expect_int_clamp_max
use std.test.expect_int_clamp_min as expect_int_clamp_min
use std.test.expect_int_clamped as expect_int_clamped
use std.test.expect_int_clamped_bounds as expect_int_clamped_bounds
use std.test.expect_int_compare as expect_int_compare
use std.test.expect_int_distance_to_bounds as expect_int_distance_to_bounds
use std.test.expect_int_distance_to_range as expect_int_distance_to_range
use std.test.expect_int_divisible_by as expect_int_divisible_by
use std.test.expect_int_eq as expect_int_eq
use std.test.expect_int_even as expect_int_even
use std.test.expect_int_exclusive_between as expect_int_exclusive_between
use std.test.expect_int_exclusive_between_bounds as expect_int_exclusive_between_bounds
use std.test.expect_int_factor_of as expect_int_factor_of
use std.test.expect_int_option_none as expect_int_option_none
use std.test.expect_int_option_ok_or as expect_int_option_ok_or
use std.test.expect_int_option_ok_or_err as expect_int_option_ok_or_err
use std.test.expect_int_option_or as expect_int_option_or
use std.test.expect_int_option_some as expect_int_option_some
use std.test.expect_int_result_err as expect_int_result_err
use std.test.expect_int_result_error_none as expect_int_result_error_none
use std.test.expect_int_result_error_some as expect_int_result_error_some
use std.test.expect_int_result_ok as expect_int_result_ok
use std.test.expect_int_result_or as expect_int_result_or
use std.test.expect_int_result_to_option_none as expect_int_result_to_option_none
use std.test.expect_int_result_to_option_some as expect_int_result_to_option_some
use std.test.expect_int_ge as expect_int_ge
use std.test.expect_int_gt as expect_int_gt
use std.test.expect_int_has_remainder as expect_int_has_remainder
use std.test.expect_int_le as expect_int_le
use std.test.expect_int_lower_bound as expect_int_lower_bound
use std.test.expect_int_lt as expect_int_lt
use std.test.expect_int_max as expect_int_max
use std.test.expect_int_min as expect_int_min
use std.test.expect_int_ne as expect_int_ne
use std.test.expect_int_negative as expect_int_negative
use std.test.expect_int_nonnegative as expect_int_nonnegative
use std.test.expect_int_nonpositive as expect_int_nonpositive
use std.test.expect_int_not_within as expect_int_not_within
use std.test.expect_int_odd as expect_int_odd
use std.test.expect_int_outside as expect_int_outside
use std.test.expect_int_outside_bounds as expect_int_outside_bounds
use std.test.expect_int_positive as expect_int_positive
use std.test.expect_int_quotient_or_zero as expect_int_quotient_or_zero
use std.test.expect_int_range_span as expect_int_range_span
use std.test.expect_int_remainder_or_zero as expect_int_remainder_or_zero
use std.test.expect_int_sign as expect_int_sign
use std.test.expect_int_upper_bound as expect_int_upper_bound
use std.test.expect_int_within as expect_int_within
use std.test.expect_ne as expect_ne
use std.test.expect_nonzero as expect_nonzero
use std.test.expect_option_none as expect_option_none
use std.test.expect_option_ok_or as expect_option_ok_or
use std.test.expect_option_ok_or_err as expect_option_ok_or_err
use std.test.expect_option_or as expect_option_or
use std.test.expect_option_some as expect_option_some
use std.test.expect_result_err as expect_result_err
use std.test.expect_result_error as expect_result_error
use std.test.expect_result_error_none as expect_result_error_none
use std.test.expect_result_error_some as expect_result_error_some
use std.test.expect_result_ok as expect_result_ok
use std.test.expect_result_or as expect_result_or
use std.test.expect_result_to_option_none as expect_result_to_option_none
use std.test.expect_result_to_option_some as expect_result_to_option_some
use std.test.expect_status_failed as expect_status_failed
use std.test.expect_status_ok as expect_status_ok
use std.test.expect_true as expect_true
use std.test.expect_zero as expect_zero
use std.test.is_status_failed as is_status_failed
use std.test.is_status_ok as is_status_ok
use std.test.merge_status as merge_status
use std.test.merge_statuses as merge_statuses
use std.option.Option as Option
use std.result.Result as Result

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

fn sum_statuses[N](statuses: [Int; N]) -> Int {
    var total = 0
    for status in statuses {
        total = total + status
    }
    return total
}

fn main() -> Int {
    let generic_pass = sum_statuses([check_int(expect_eq(7, 7), 0), check_int(expect_eq(true, true), 0), check_int(expect_ne("left", "right"), 0), check_int(expect_array_first(["red", "blue", "green"], "red"), 0)])
    let generic_array_pass = sum_statuses([check_int(expect_array_last(["red", "blue", "green"], "green"), 0), check_int(expect_array_at(["red", "blue", "green"], 1, "none", "blue"), 0), check_int(expect_array_contains(["red", "blue", "green"], "blue", true), 0), check_int(expect_array_count(["red", "blue", "red"], "red", 2), 0)])
    let generic_array_eq_status = sum_statuses([check_int(expect_array_eq(["red", "blue", "green"], ["red", "blue", "green"]), 0), check_int(expect_array_eq([1, 2, 3, 4], [1, 2, 3, 4]), 0), check_int(expect_array_eq([true, false, true], [true, false, true]), 0), 0])
    let generic_failure = sum_statuses([check_int(expect_eq(7, 8), 1), check_int(expect_ne(true, true), 1), check_int(expect_array_at(["red", "blue"], 8, "none", "blue"), 1), check_int(expect_array_contains(["red", "blue"], "green", true), 1)])
    let bool_pass = sum_statuses([check_int(expect_true(true), 0), check_int(expect_false(false), 0), check_int(expect_bool_eq(true, true), 0), check_int(expect_bool_ne(true, false), 0)])
    let bool_logic_pass = sum_statuses([check_int(expect_bool_not(false, true), 0), check_int(expect_bool_and(true, false, false), 0), check_int(expect_bool_or(false, true, true), 0), check_int(expect_bool_xor(true, true, false), 0)])
    let bool_failure = sum_statuses([check_int(expect_true(false), 1), check_int(expect_false(true), 1), check_int(expect_bool_eq(true, false), 1), check_int(expect_bool_ne(true, true), 1)])
    let bool_logic_failure = sum_statuses([check_int(expect_bool_not(false, false), 1), check_int(expect_bool_and(true, false, true), 1), check_int(expect_bool_or(false, false, true), 1), check_int(expect_bool_xor(true, false, false), 1)])
    let bool_aggregate_pass = sum_statuses([check_int(expect_bool_array_all([true, true, true], true), 0), check_int(expect_bool_array_all([true, true, false, true], false), 0), check_int(expect_bool_array_any([false, false, true], true), 0), check_int(expect_bool_array_any([false, false, false, false], false), 0)])
    let bool_large_aggregate_pass = sum_statuses([check_int(expect_bool_array_all([true, true, true, true, true], true), 0), check_int(expect_bool_array_any([false, false, false, false, true], true), 0), 0, 0])
    let bool_none_pass = sum_statuses([check_int(expect_bool_array_none([false, false, false], true), 0), check_int(expect_bool_array_none([false, false, true, false], false), 0), check_int(expect_bool_array_none([false, false, false, false, false], true), 0), 0])
    let bool_conversion_pass = sum_statuses([check_int(expect_bool_to_int(true, 1), 0), check_int(expect_bool_to_int(false, 0), 0), 0, 0])
    let bool_aggregate_failure = sum_statuses([check_int(expect_bool_array_all([true, true, false], true), 1), check_int(expect_bool_array_all([true, true, true, true], false), 1), check_int(expect_bool_array_any([false, false, false], true), 1), check_int(expect_bool_array_any([false, false, true, false], false), 1)])
    let bool_large_aggregate_failure = sum_statuses([check_int(expect_bool_array_all([true, true, true, true, false], true), 1), check_int(expect_bool_array_any([false, false, false, false, false], true), 1), 0, 0])
    let bool_none_failure = sum_statuses([check_int(expect_bool_array_none([false, true, false], true), 1), check_int(expect_bool_array_none([false, false, false, false], false), 1), check_int(expect_bool_array_none([false, false, true, false, false], true), 1), 0])
    let bool_conversion_failure = sum_statuses([check_int(expect_bool_to_int(false, 1), 1), check_int(expect_bool_to_int(true, 0), 1), 0, 0])
    let int_order_pass = sum_statuses([check_int(expect_int_eq(8, 8), 0), check_int(expect_int_ne(8, 9), 0), check_int(expect_int_gt(9, 8), 0), check_int(expect_int_ge(8, 8), 0)])
    let int_boundary_pass = sum_statuses([check_int(expect_int_lt(7, 8), 0), check_int(expect_int_le(8, 8), 0), check_int(expect_zero(0), 0), check_int(expect_nonzero(1), 0)])
    let int_order_failure = sum_statuses([check_int(expect_int_eq(8, 9), 1), check_int(expect_int_ne(8, 8), 1), check_int(expect_int_gt(8, 8), 1), check_int(expect_int_ge(7, 8), 1)])
    let int_boundary_failure = sum_statuses([check_int(expect_int_lt(8, 8), 1), check_int(expect_int_le(9, 8), 1), check_int(expect_zero(1), 1), check_int(expect_nonzero(0), 1)])
    let range_pass = sum_statuses([check_int(expect_int_between(5, 3, 9), 0), check_int(expect_int_exclusive_between(5, 3, 9), 0), check_int(expect_int_outside(2, 3, 9), 0), check_int(expect_int_between_bounds(5, 9, 3), 0)])
    let bounds_pass = sum_statuses([check_int(expect_int_exclusive_between_bounds(5, 9, 3), 0), check_int(expect_int_outside_bounds(2, 9, 3), 0), check_int(expect_int_array_ascending([3, 3, 9]), 0), check_int(expect_int_array_descending([9, 9, 3]), 0)])
    let order_pass = sum_statuses([check_int(expect_int_array_strictly_ascending([3, 5, 9]), 0), check_int(expect_int_array_strictly_descending([9, 5, 3]), 0), 0, 0])
    let ascending_multi_pass = sum_statuses([check_int(expect_int_array_ascending([3, 5, 5, 9]), 0), check_int(expect_int_array_ascending([3, 5, 5, 9, 10]), 0), check_int(expect_int_array_strictly_ascending([3, 5, 7, 9]), 0), check_int(expect_int_array_strictly_ascending([3, 5, 7, 9, 11]), 0)])
    let descending_multi_pass = sum_statuses([check_int(expect_int_array_descending([9, 7, 7, 3]), 0), check_int(expect_int_array_descending([11, 9, 7, 7, 3]), 0), check_int(expect_int_array_strictly_descending([9, 7, 5, 3]), 0), check_int(expect_int_array_strictly_descending([11, 9, 7, 5, 3]), 0)])
    let range_failure = sum_statuses([check_int(expect_int_between(2, 3, 9), 1), check_int(expect_int_exclusive_between(3, 3, 9), 1), check_int(expect_int_outside(5, 3, 9), 1), check_int(expect_int_between_bounds(10, 9, 3), 1)])
    let bounds_failure = sum_statuses([check_int(expect_int_exclusive_between_bounds(9, 9, 3), 1), check_int(expect_int_outside_bounds(5, 9, 3), 1), check_int(expect_int_array_ascending([9, 5, 3]), 1), check_int(expect_int_array_descending([3, 9, 5]), 1)])
    let order_failure = sum_statuses([check_int(expect_int_array_strictly_ascending([3, 3, 9]), 1), check_int(expect_int_array_strictly_descending([9, 9, 3]), 1), 0, 0])
    let ascending_multi_failure = sum_statuses([check_int(expect_int_array_ascending([3, 9, 5, 7]), 1), check_int(expect_int_array_ascending([3, 5, 9, 7, 10]), 1), check_int(expect_int_array_strictly_ascending([3, 5, 5, 9]), 1), check_int(expect_int_array_strictly_ascending([3, 5, 7, 9, 9]), 1)])
    let descending_multi_failure = sum_statuses([check_int(expect_int_array_descending([9, 3, 7, 5]), 1), check_int(expect_int_array_descending([11, 9, 3, 7, 5]), 1), check_int(expect_int_array_strictly_descending([9, 7, 7, 3]), 1), check_int(expect_int_array_strictly_descending([11, 9, 7, 7, 3]), 1)])
    let transform_core_pass = sum_statuses([check_int(expect_int_abs(0 - 7, 7), 0), check_int(expect_int_abs_diff(3, 9, 6), 0), check_int(expect_int_range_span(9, 3, 6), 0), check_int(expect_int_lower_bound(9, 3, 3), 0)])
    let transform_bound_pass = sum_statuses([check_int(expect_int_upper_bound(9, 3, 9), 0), 0, 0, 0])
    let transform_core_failure = sum_statuses([check_int(expect_int_abs(0 - 7, 0 - 7), 1), check_int(expect_int_abs_diff(3, 9, 5), 1), check_int(expect_int_range_span(9, 3, 5), 1), check_int(expect_int_lower_bound(9, 3, 9), 1)])
    let transform_bound_failure = sum_statuses([check_int(expect_int_upper_bound(9, 3, 3), 1), 0, 0, 0])
    let transform_pass = sum_statuses([check_int(expect_int_clamped(12, 3, 9, 9), 0), check_int(expect_int_clamped_bounds(2, 9, 3, 3), 0), check_int(expect_int_distance_to_range(2, 3, 9, 1), 0), check_int(expect_int_distance_to_bounds(10, 9, 3, 1), 0)])
    let transform_failure = sum_statuses([check_int(expect_int_clamped(12, 3, 9, 12), 1), check_int(expect_int_clamped_bounds(2, 9, 3, 2), 1), check_int(expect_int_distance_to_range(5, 3, 9, 1), 1), check_int(expect_int_distance_to_bounds(5, 9, 3, 1), 1)])
    let transform_clamp_pass = sum_statuses([check_int(expect_int_clamp_min(19, 20, 20), 0), check_int(expect_int_clamp_max(23, 22, 22), 0), 0, 0])
    let transform_clamp_failure = sum_statuses([check_int(expect_int_clamp_min(19, 20, 19), 1), check_int(expect_int_clamp_max(23, 22, 23), 1), 0, 0])
    let aggregate_pass = sum_statuses([check_int(expect_int_array_sum([2, 3, 4], 9), 0), check_int(expect_int_array_sum([2, 3, 4, 5], 14), 0), check_int(expect_int_array_product([2, 3, 4], 24), 0), check_int(expect_int_array_product([2, 3, 4, 5], 120), 0)])
    let aggregate_large_pass = sum_statuses([check_int(expect_int_array_sum([2, 3, 4, 5, 6], 20), 0), check_int(expect_int_array_product([2, 3, 4, 5, 6], 720), 0), 0, 0])
    let average_pass = sum_statuses([check_int(expect_int_array_average([5, 8], 6), 0), check_int(expect_int_array_average([3, 6, 9], 6), 0), check_int(expect_int_array_average([2, 4, 6, 8], 5), 0), check_int(expect_int_array_average([2, 4, 6, 8, 10], 6), 0)])
    let aggregate_failure = sum_statuses([check_int(expect_int_array_sum([2, 3, 4], 10), 1), check_int(expect_int_array_sum([2, 3, 4, 5], 15), 1), check_int(expect_int_array_product([2, 3, 4], 25), 1), check_int(expect_int_array_product([2, 3, 4, 5], 121), 1)])
    let aggregate_large_failure = sum_statuses([check_int(expect_int_array_sum([2, 3, 4, 5, 6], 21), 1), check_int(expect_int_array_product([2, 3, 4, 5, 6], 721), 1), 0, 0])
    let average_failure = sum_statuses([check_int(expect_int_array_average([5, 8], 7), 1), check_int(expect_int_array_average([3, 6, 9], 7), 1), check_int(expect_int_array_average([2, 4, 6, 8], 6), 1), check_int(expect_int_array_average([2, 4, 6, 8, 10], 7), 1)])
    let division_pass = sum_statuses([check_int(expect_int_quotient_or_zero(21, 7, 3), 0), check_int(expect_int_quotient_or_zero(21, 0, 0), 0), check_int(expect_int_remainder_or_zero(22, 7, 1), 0), check_int(expect_int_remainder_or_zero(22, 0, 0), 0)])
    let division_bool_pass = sum_statuses([check_int(expect_int_has_remainder(22, 7), 0), check_int(expect_int_factor_of(7, 21), 0), 0, 0])
    let division_failure = sum_statuses([check_int(expect_int_quotient_or_zero(21, 7, 4), 1), check_int(expect_int_remainder_or_zero(22, 7, 2), 1), check_int(expect_int_has_remainder(21, 7), 1), check_int(expect_int_factor_of(0, 21), 1)])
    let division_zero_failure = sum_statuses([check_int(expect_int_quotient_or_zero(21, 0, 1), 1), check_int(expect_int_remainder_or_zero(21, 0, 1), 1), 0, 0])
    let compare_sign_pass = sum_statuses([check_int(expect_int_sign(0 - 5, 0 - 1), 0), check_int(expect_int_sign(0, 0), 0), check_int(expect_int_sign(5, 1), 0), check_int(expect_int_compare(3, 9, 0 - 1), 0)])
    let compare_sign_more_pass = sum_statuses([check_int(expect_int_compare(9, 9, 0), 0), check_int(expect_int_compare(9, 3, 1), 0), 0, 0])
    let compare_sign_failure = sum_statuses([check_int(expect_int_sign(5, 0 - 1), 1), check_int(expect_int_compare(9, 9, 1), 1), check_int(expect_int_compare(3, 9, 1), 1), 0])
    let extrema_pass = sum_statuses([check_int(expect_int_max(20, 22, 22), 0), check_int(expect_int_min(20, 22, 20), 0), check_int(expect_int_array_max([20, 22, 21], 22), 0), check_int(expect_int_array_min([20, 22, 21], 20), 0)])
    let extrema_median_pass = sum_statuses([check_int(expect_int_array_max([20, 22, 21, 19], 22), 0), check_int(expect_int_array_min([20, 22, 21, 19], 19), 0), check_int(expect_eq(median_ints([22, 20, 21]), 21), 0), 0])
    let extrema_large_pass = sum_statuses([check_int(expect_int_array_max([20, 22, 21, 19, 23], 23), 0), check_int(expect_int_array_min([20, 22, 21, 19, 23], 19), 0), 0, 0])
    let extrema_failure = sum_statuses([check_int(expect_int_max(20, 22, 20), 1), check_int(expect_int_min(20, 22, 22), 1), check_int(expect_int_array_max([20, 22, 21], 21), 1), check_int(expect_int_array_min([20, 22, 21], 21), 1)])
    let extrema_median_failure = sum_statuses([check_int(expect_int_array_max([20, 22, 21, 19], 21), 1), check_int(expect_int_array_min([20, 22, 21, 19], 20), 1), check_int(expect_eq(median_ints([22, 20, 21]), 22), 1), 0])
    let extrema_large_failure = sum_statuses([check_int(expect_int_array_max([20, 22, 21, 19, 23], 22), 1), check_int(expect_int_array_min([20, 22, 21, 19, 23], 20), 1), 0, 0])
    let number_pass = sum_statuses([check_int(expect_int_even(8), 0), check_int(expect_int_odd(9), 0), check_int(expect_int_divisible_by(21, 7), 0), check_int(expect_int_within(11, 10, 1), 0)])
    let sign_pass = sum_statuses([check_int(expect_int_not_within(12, 10, 1), 0), check_int(expect_int_positive(1), 0), check_int(expect_int_negative(0 - 1), 0), check_int(expect_int_nonnegative(0), 0)])
    let number_failure = sum_statuses([check_int(expect_int_even(9), 1), check_int(expect_int_odd(8), 1), check_int(expect_int_divisible_by(21, 0), 1), check_int(expect_int_within(12, 10, 1), 1)])
    let sign_failure = sum_statuses([check_int(expect_int_not_within(10, 10, 0), 1), check_int(expect_int_positive(0), 1), check_int(expect_int_negative(0), 1), check_int(expect_int_nonnegative(0 - 1), 1)])
    let status_bool = sum_statuses([check_bool(is_status_ok(0), true), check_bool(is_status_ok(1), false), check_bool(is_status_failed(1), true), check_bool(is_status_failed(0), false)])
    let status_merge = sum_statuses([check_int(merge_status(1, 2), 3), check_int(merge_statuses([1, 2, 3]), 6), check_int(merge_statuses([1, 2, 3, 4]), 10), check_int(merge_statuses([1, 2, 3, 4, 5]), 15)])
    let status_merge_large = sum_statuses([check_int(merge_statuses([1, 2, 3, 4, 5, 6]), 21), check_int(merge_statuses([1, 2, 3, 4, 5, 6, 7]), 28), check_int(expect_bool_implies(false, false), 0), 0])
    let status_expect = sum_statuses([check_int(expect_status_ok(0), 0), check_int(expect_status_ok(1), 1), check_int(expect_status_failed(1), 0), check_int(expect_status_failed(0), 1)])
    let sign_boundary = sum_statuses([check_int(expect_int_nonpositive(0), 0), check_int(expect_int_nonpositive(1), 1), check_int(expect_bool_implies(true, false), 1), 0])
    let some_string_option: Option[String] = Option.Some("ready")
    let none_string_option: Option[String] = Option.None
    let ok_string_result: Result[String, String] = Result.Ok("ready")
    let err_string_result: Result[String, String] = Result.Err("denied")
    let fallback_string_result: Result[String, String] = Result.Ok("fallback")
    let generic_option_status = sum_statuses([check_int(expect_option_some(some_string_option, "ready"), 0), check_int(expect_option_none(none_string_option), 0), check_int(expect_option_or(none_string_option, Option.Some("fallback"), "fallback"), 0), check_int(expect_option_ok_or(some_string_option, "missing", "ready"), 0), check_int(expect_option_ok_or_err(none_string_option, "missing"), 0)])
    let generic_option_failure = sum_statuses([check_int(expect_option_some(some_string_option, "other"), 1), check_int(expect_option_none(some_string_option), 1), check_int(expect_option_or(some_string_option, none_string_option, "other"), 1), check_int(expect_option_ok_or(some_string_option, "missing", "other"), 1), check_int(expect_option_ok_or_err(some_string_option, "missing"), 1)])
    let generic_result_status = sum_statuses([check_int(expect_result_ok(ok_string_result, "ready"), 0), check_int(expect_result_err(err_string_result, "denied"), 0), check_int(expect_result_or(err_string_result, fallback_string_result, "fallback"), 0), check_int(expect_result_error(ok_string_result, "fallback-error", "fallback-error"), 0), check_int(expect_result_to_option_some(ok_string_result, "ready"), 0), check_int(expect_result_to_option_none(err_string_result), 0), check_int(expect_result_error_some(err_string_result, "denied"), 0), check_int(expect_result_error_none(ok_string_result), 0)])
    let generic_result_failure = sum_statuses([check_int(expect_result_ok(ok_string_result, "other"), 1), check_int(expect_result_err(ok_string_result, "denied"), 1), check_int(expect_result_or(ok_string_result, fallback_string_result, "fallback"), 1), check_int(expect_result_error(err_string_result, "fallback-error", "other"), 1), check_int(expect_result_to_option_some(err_string_result, "ready"), 1), check_int(expect_result_to_option_none(ok_string_result), 1), check_int(expect_result_error_some(err_string_result, "other"), 1), check_int(expect_result_error_none(err_string_result), 1)])
    let option_status = sum_statuses([check_int(expect_int_option_some(Option.Some(7), 7), 0), check_int(expect_int_option_none(Option.None), 0), check_int(expect_bool_option_some(Option.Some(true), true), 0), check_int(expect_bool_option_none(Option.None), 0)])
    let option_or_status = sum_statuses([check_int(expect_int_option_or(Option.None, Option.Some(9), 9), 0), check_int(expect_bool_option_or(Option.None, Option.Some(false), false), 0), 0, 0])
    let option_failure = sum_statuses([check_int(expect_int_option_some(Option.Some(7), 8), 1), check_int(expect_int_option_none(Option.Some(7)), 1), check_int(expect_bool_option_some(Option.Some(true), false), 1), check_int(expect_bool_option_or(Option.Some(true), Option.None, false), 1)])
    let result_status = sum_statuses([check_int(expect_int_result_ok(Result.Ok(7), 7), 0), check_int(expect_int_result_err(Result.Err(3), 3), 0), check_int(expect_bool_result_ok(Result.Ok(true), true), 0), check_int(expect_bool_result_err(Result.Err(4), 4), 0)])
    let result_or_status = sum_statuses([check_int(expect_int_result_or(Result.Err(5), Result.Ok(11), 11), 0), check_int(expect_bool_result_or(Result.Err(6), Result.Ok(false), false), 0), 0, 0])
    let result_failure = sum_statuses([check_int(expect_int_result_ok(Result.Ok(7), 8), 1), check_int(expect_int_result_err(Result.Ok(7), 3), 1), check_int(expect_bool_result_ok(Result.Ok(true), false), 1), check_int(expect_bool_result_or(Result.Ok(true), Result.Err(4), false), 1)])
    let result_conversion_status = sum_statuses([check_int(expect_int_result_to_option_some(Result.Ok(14), 14), 0), check_int(expect_int_result_to_option_none(Result.Err(4)), 0), check_int(expect_bool_result_to_option_some(Result.Ok(false), false), 0), check_int(expect_bool_result_to_option_none(Result.Err(5)), 0)])
    let option_conversion_status = sum_statuses([check_int(expect_int_option_ok_or(Option.Some(14), 5, 14), 0), check_int(expect_int_option_ok_or_err(Option.None, 5), 0), check_int(expect_bool_option_ok_or(Option.Some(true), 6, true), 0), check_int(expect_bool_option_ok_or_err(Option.None, 6), 0)])
    let result_conversion_failure = sum_statuses([check_int(expect_int_result_to_option_some(Result.Err(4), 14), 1), check_int(expect_int_result_to_option_none(Result.Ok(14)), 1), check_int(expect_bool_result_to_option_some(Result.Ok(false), true), 1), check_int(expect_bool_result_to_option_none(Result.Ok(false)), 1)])
    let option_conversion_failure = sum_statuses([check_int(expect_int_option_ok_or(Option.Some(14), 5, 15), 1), check_int(expect_int_option_ok_or_err(Option.Some(14), 5), 1), check_int(expect_bool_option_ok_or(Option.Some(true), 6, false), 1), check_int(expect_bool_option_ok_or_err(Option.Some(true), 6), 1)])
    let result_error_status = sum_statuses([check_int(expect_int_result_error_some(Result.Err(0), 0), 0), check_int(expect_int_result_error_none(Result.Ok(14)), 0), check_int(expect_bool_result_error_some(Result.Err(0), 0), 0), check_int(expect_bool_result_error_none(Result.Ok(false)), 0)])
    let result_error_failure = sum_statuses([check_int(expect_int_result_error_some(Result.Ok(14), 0), 1), check_int(expect_int_result_error_none(Result.Err(0)), 1), check_int(expect_bool_result_error_some(Result.Err(0), 1), 1), check_int(expect_bool_result_error_none(Result.Err(0)), 1)])
    let none_bool_option: Option[Bool] = Option.None
    let result_conversion_more_status = sum_statuses([check_int(expect_int_result_to_option_some(Result.Ok(31), 31), 0), check_int(expect_int_result_to_option_none(Result.Err(8)), 0), check_int(expect_bool_result_to_option_some(Result.Ok(false), false), 0), check_int(expect_bool_result_to_option_none(Result.Err(9)), 0)])
    let result_error_option_status = sum_statuses([check_int(expect_int_result_error_some(Result.Err(0), 0), 0), check_int(expect_int_result_error_none(Result.Ok(31)), 0), check_int(expect_bool_result_error_some(Result.Err(0), 0), 0), check_int(expect_bool_result_error_none(Result.Ok(false)), 0)])
    let option_conversion_more_status = sum_statuses([check_int(expect_int_option_ok_or(Option.Some(31), 8, 31), 0), check_int(expect_int_option_ok_or_err(Option.None, 8), 0), check_int(expect_bool_option_ok_or(Option.Some(true), 9, true), 0), check_int(expect_bool_option_ok_or_err(none_bool_option, 9), 0)])
    let generic_array_access_status = sum_statuses([check_int(expect_int_array_first([9, 8, 7, 6], 9), 0), check_int(expect_int_array_last([9, 8, 7, 6], 6), 0), check_int(expect_bool_array_first([false, true, true], false), 0), check_int(expect_bool_array_last([false, true, true], true), 0)])
    let generic_array_at_status = sum_statuses([check_int(expect_int_array_at([2, 3, 4, 5], 2, 99, 4), 0), check_int(expect_int_array_at([2, 3, 4], 8, 99, 99), 0), check_int(expect_bool_array_at([true, false, true], 1, true, false), 0), check_int(expect_bool_array_at([true, false, true], 8, false, false), 0)])
    let generic_array_query_status = sum_statuses([check_int(expect_int_array_contains([2, 3, 4, 5], 3, true), 0), check_int(expect_int_array_count([2, 3, 2, 2, 4], 2, 3), 0), check_int(expect_bool_array_contains([true, false, true], false, true), 0), check_int(expect_bool_array_count([true, false, true, false], false, 2), 0)])
    let generic_array_aggregate_status = sum_statuses([check_int(expect_int_array_sum([2, 3, 4, 5], 14), 0), check_int(expect_int_array_product([2, 3, 4], 24), 0), check_int(expect_int_array_max([3, 9, 5, 7], 9), 0), check_int(expect_int_array_min([3, 9, 5, 7], 3), 0)])
    let generic_array_bool_aggregate_status = sum_statuses([check_int(expect_bool_array_all([true, true, true], true), 0), check_int(expect_bool_array_any([false, false, true, false], true), 0), check_int(expect_bool_array_none([false, false, false, false, false], true), 0), 0])
    let generic_array_reverse_status = sum_statuses([check_int(expect_array_reverse(["north", "east", "south"], ["south", "east", "north"]), 0), check_int(expect_int_array_reverse([2, 3, 4, 5], [5, 4, 3, 2]), 0), check_int(expect_bool_array_reverse([true, false, false], [false, false, true]), 0), check_int(expect_bool_array_reverse([true, false, true, false, false], [false, false, true, false, true]), 0)])
    let array_failure = sum_statuses([check_int(expect_int_array_first([2, 3, 4], 3), 1), check_int(expect_bool_array_last([false, true, false, true, false], true), 1), check_int(expect_int_array_sum([2, 3, 4, 5, 6], 21), 1), check_int(expect_bool_array_none([false, false, false, false], false), 1)])
    let array_at_failure = sum_statuses([check_int(expect_int_array_at([2, 3, 4], 1, 99, 2), 1), check_int(expect_int_array_at([2, 3, 4, 5, 6], 8, 99, 6), 1), check_int(expect_bool_array_at([true, false, true], 1, true, true), 1), check_int(expect_bool_array_at([true, false, true, false, true], 8, false, true), 1)])
    let array_transform_failure = sum_statuses([check_int(expect_array_reverse(["north", "east", "south"], ["south", "west", "north"]), 1), check_int(expect_int_array_reverse([2, 3, 4], [2, 3, 4]), 2), check_int(expect_bool_array_reverse([true, false, true, false, false], [true, false, true, false, false]), 2), check_int(expect_array_eq(["a", "b", "c"], ["a", "x", "c"]), 1)])
    let array_query_failure = sum_statuses([check_int(expect_int_array_contains([2, 3, 4, 5, 6], 7, true), 1), check_int(expect_int_array_count([2, 3, 2], 2, 1), 1), check_int(expect_bool_array_contains([false, false, false, false], true, true), 1), check_int(expect_bool_array_count([true, false, true, false], false, 1), 1)])
    let bool_status = sum_statuses([bool_pass + bool_aggregate_pass + bool_large_aggregate_pass + bool_conversion_pass, bool_logic_pass + bool_none_pass, bool_failure + bool_aggregate_failure + bool_large_aggregate_failure + bool_conversion_failure, bool_logic_failure + bool_none_failure])
    let int_status = sum_statuses([int_order_pass, int_boundary_pass, int_order_failure, int_boundary_failure])
    let range_status = sum_statuses([range_pass, bounds_pass, range_failure, bounds_failure])
    let order_status = sum_statuses([order_pass + ascending_multi_pass + descending_multi_pass, order_failure + ascending_multi_failure + descending_multi_failure, compare_sign_pass, compare_sign_more_pass + compare_sign_failure])
    let number_status = sum_statuses([number_pass, sign_pass, number_failure, sign_failure])
    let status_status = sum_statuses([status_bool, status_merge, status_merge_large, status_expect])
    return check_int(sum_statuses([bool_status, int_status, range_status, sum_statuses([order_status, number_status, status_status, sum_statuses([sign_boundary, transform_pass + transform_core_pass + transform_clamp_pass, transform_failure + transform_core_failure + transform_clamp_failure, sum_statuses([aggregate_pass + aggregate_large_pass + extrema_pass + division_pass, average_pass + extrema_median_pass + extrema_large_pass + division_bool_pass + transform_bound_pass, aggregate_failure + aggregate_large_failure + extrema_failure + division_failure, average_failure + extrema_median_failure + extrema_large_failure + division_zero_failure + transform_bound_failure])])])]), 0) + generic_pass + generic_array_pass + generic_array_eq_status + generic_failure + generic_option_status + generic_option_failure + generic_result_status + generic_result_failure + option_status + option_or_status + option_failure + result_status + result_or_status + result_failure + result_conversion_status + option_conversion_status + result_conversion_failure + option_conversion_failure + result_error_status + result_error_failure + result_conversion_more_status + result_error_option_status + option_conversion_more_status + generic_array_access_status + generic_array_at_status + generic_array_query_status + generic_array_aggregate_status + generic_array_bool_aggregate_status + generic_array_reverse_status + array_failure + array_at_failure + array_transform_failure + array_query_failure
}
