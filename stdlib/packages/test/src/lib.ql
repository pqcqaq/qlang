package std.test

use std.core.abs_diff_int as abs_diff_int
use std.core.abs_int as abs_int
use std.core.all_bools as all_bools
use std.core.and_bool as and_bool
use std.core.any_bools as any_bools
use std.core.average_ints as average_ints
use std.core.bool_to_int as bool_to_int
use std.core.clamp_bounds_int as clamp_bounds_int
use std.core.clamp_int as clamp_int
use std.core.clamp_max_int as clamp_max_int
use std.core.clamp_min_int as clamp_min_int
use std.core.compare_int as compare_int
use std.core.distance_to_bounds_int as distance_to_bounds_int
use std.core.distance_to_range_int as distance_to_range_int
use std.core.implies_bool as implies_bool
use std.core.in_bounds_int as in_bounds_int
use std.core.in_exclusive_bounds_int as in_exclusive_bounds_int
use std.core.in_exclusive_range_int as in_exclusive_range_int
use std.core.in_range_int as in_range_int
use std.core.has_remainder_int as has_remainder_int
use std.core.is_ascending_ints as is_ascending_ints
use std.core.is_descending_ints as is_descending_ints
use std.core.is_divisible_by_int as is_divisible_by_int
use std.core.is_even_int as is_even_int
use std.core.is_factor_of_int as is_factor_of_int
use std.core.is_negative_int as is_negative_int
use std.core.is_not_within_int as is_not_within_int
use std.core.is_nonnegative_int as is_nonnegative_int
use std.core.is_nonpositive_int as is_nonpositive_int
use std.core.is_odd_int as is_odd_int
use std.core.is_outside_bounds_int as is_outside_bounds_int
use std.core.is_outside_range_int as is_outside_range_int
use std.core.is_positive_int as is_positive_int
use std.core.is_strictly_ascending_ints as is_strictly_ascending_ints
use std.core.is_strictly_descending_ints as is_strictly_descending_ints
use std.core.is_within_int as is_within_int
use std.core.lower_bound_int as lower_bound_int
use std.core.max_int as max_int
use std.core.max_ints as max_ints
use std.core.min_int as min_int
use std.core.min_ints as min_ints
use std.core.none_bools as none_bools
use std.core.not_bool as not_bool
use std.core.or_bool as or_bool
use std.core.product_ints as product_ints
use std.core.quotient_or_zero_int as quotient_or_zero_int
use std.core.range_span_int as range_span_int
use std.core.remainder_or_zero_int as remainder_or_zero_int
use std.core.sign_int as sign_int
use std.core.sum_ints as sum_ints
use std.core.upper_bound_int as upper_bound_int
use std.core.xor_bool as xor_bool
use std.array.at_array_or as at_array_or
use std.array.contains_array as contains_array
use std.array.count_array as count_array
use std.array.count_mismatches_array as count_mismatches_array
use std.array.first_array as first_array
use std.array.last_array as last_array
use std.array.reverse_array as reverse_array
use std.option.Option as Option
use std.option.or_option as or_option
use std.result.Result as Result
use std.result.error_or as error_or
use std.result.error_to_option as error_to_option
use std.result.ok_or as ok_or
use std.result.or_result as or_result
use std.result.to_option as to_option

pub fn expect_eq[T](actual: T, expected: T) -> Int {
    if actual == expected {
        return 0
    }
    return 1
}

pub fn expect_ne[T](actual: T, unexpected: T) -> Int {
    if actual != unexpected {
        return 0
    }
    return 1
}

pub fn expect_array_first[T, N](values: [T; N], expected: T) -> Int {
    if first_array(values) == expected {
        return 0
    }
    return 1
}

pub fn expect_array_last[T, N](values: [T; N], expected: T) -> Int {
    if last_array(values) == expected {
        return 0
    }
    return 1
}

pub fn expect_array_at[T, N](values: [T; N], index: Int, fallback: T, expected: T) -> Int {
    if at_array_or(values, index, fallback) == expected {
        return 0
    }
    return 1
}

pub fn expect_array_contains[T, N](values: [T; N], needle: T, expected: Bool) -> Int {
    if contains_array(values, needle) == expected {
        return 0
    }
    return 1
}

pub fn expect_array_count[T, N](values: [T; N], needle: T, expected: Int) -> Int {
    if count_array(values, needle) == expected {
        return 0
    }
    return 1
}

pub fn expect_array_eq[T, N](actual: [T; N], expected: [T; N]) -> Int {
    return count_mismatches_array(actual, expected)
}

pub fn expect_true(value: Bool) -> Int {
    if value {
        return 0
    }
    return 1
}

pub fn expect_false(value: Bool) -> Int {
    if value {
        return 1
    }
    return 0
}

pub fn expect_bool_not(value: Bool, expected: Bool) -> Int {
    if not_bool(value) == expected {
        return 0
    }
    return 1
}

pub fn expect_bool_and(left: Bool, right: Bool, expected: Bool) -> Int {
    if and_bool(left, right) == expected {
        return 0
    }
    return 1
}

pub fn expect_bool_or(left: Bool, right: Bool, expected: Bool) -> Int {
    if or_bool(left, right) == expected {
        return 0
    }
    return 1
}

pub fn expect_bool_xor(left: Bool, right: Bool, expected: Bool) -> Int {
    if xor_bool(left, right) == expected {
        return 0
    }
    return 1
}

pub fn expect_bool_to_int(value: Bool, expected: Int) -> Int {
    if bool_to_int(value) == expected {
        return 0
    }
    return 1
}

pub fn expect_int_array_sum[N](values: [Int; N], expected: Int) -> Int {
    if sum_ints(values) == expected {
        return 0
    }
    return 1
}

pub fn expect_int_array_product[N](values: [Int; N], expected: Int) -> Int {
    if product_ints(values) == expected {
        return 0
    }
    return 1
}

pub fn expect_int_array_average[N](values: [Int; N], expected: Int) -> Int {
    if average_ints(values) == expected {
        return 0
    }
    return 1
}

pub fn expect_int_array_max[N](values: [Int; N], expected: Int) -> Int {
    if max_ints(values) == expected {
        return 0
    }
    return 1
}

pub fn expect_int_array_min[N](values: [Int; N], expected: Int) -> Int {
    if min_ints(values) == expected {
        return 0
    }
    return 1
}

pub fn expect_int_array_ascending[N](values: [Int; N]) -> Int {
    if is_ascending_ints(values) {
        return 0
    }
    return 1
}

pub fn expect_int_array_strictly_ascending[N](values: [Int; N]) -> Int {
    if is_strictly_ascending_ints(values) {
        return 0
    }
    return 1
}

pub fn expect_int_array_descending[N](values: [Int; N]) -> Int {
    if is_descending_ints(values) {
        return 0
    }
    return 1
}

pub fn expect_int_array_strictly_descending[N](values: [Int; N]) -> Int {
    if is_strictly_descending_ints(values) {
        return 0
    }
    return 1
}

pub fn expect_bool_array_all[N](values: [Bool; N], expected: Bool) -> Int {
    if all_bools(values) == expected {
        return 0
    }
    return 1
}

pub fn expect_bool_array_any[N](values: [Bool; N], expected: Bool) -> Int {
    if any_bools(values) == expected {
        return 0
    }
    return 1
}

pub fn expect_bool_array_none[N](values: [Bool; N], expected: Bool) -> Int {
    if none_bools(values) == expected {
        return 0
    }
    return 1
}

pub fn expect_array_reverse[T, N](values: [T; N], expected: [T; N]) -> Int {
    var status = 0
    var index = 0
    for value in reverse_array(values) {
        if value != expected[index] {
            status = status + 1
        };
        index = index + 1
    }
    return status
}

pub fn expect_int_gt(actual: Int, threshold: Int) -> Int {
    if actual > threshold {
        return 0
    }
    return 1
}

pub fn expect_int_ge(actual: Int, threshold: Int) -> Int {
    if actual >= threshold {
        return 0
    }
    return 1
}

pub fn expect_int_lt(actual: Int, threshold: Int) -> Int {
    if actual < threshold {
        return 0
    }
    return 1
}

pub fn expect_int_le(actual: Int, threshold: Int) -> Int {
    if actual <= threshold {
        return 0
    }
    return 1
}

pub fn expect_zero(value: Int) -> Int {
    if value == 0 {
        return 0
    }
    return 1
}

pub fn expect_nonzero(value: Int) -> Int {
    if value != 0 {
        return 0
    }
    return 1
}

pub fn expect_int_max(left: Int, right: Int, expected: Int) -> Int {
    if max_int(left, right) == expected {
        return 0
    }
    return 1
}

pub fn expect_int_min(left: Int, right: Int, expected: Int) -> Int {
    if min_int(left, right) == expected {
        return 0
    }
    return 1
}

pub fn expect_int_sign(value: Int, expected: Int) -> Int {
    if sign_int(value) == expected {
        return 0
    }
    return 1
}

pub fn expect_int_compare(left: Int, right: Int, expected: Int) -> Int {
    if compare_int(left, right) == expected {
        return 0
    }
    return 1
}

pub fn expect_int_abs(value: Int, expected: Int) -> Int {
    if abs_int(value) == expected {
        return 0
    }
    return 1
}

pub fn expect_int_abs_diff(left: Int, right: Int, expected: Int) -> Int {
    if abs_diff_int(left, right) == expected {
        return 0
    }
    return 1
}

pub fn expect_int_range_span(first_bound: Int, second_bound: Int, expected: Int) -> Int {
    if range_span_int(first_bound, second_bound) == expected {
        return 0
    }
    return 1
}

pub fn expect_int_lower_bound(first_bound: Int, second_bound: Int, expected: Int) -> Int {
    if lower_bound_int(first_bound, second_bound) == expected {
        return 0
    }
    return 1
}

pub fn expect_int_upper_bound(first_bound: Int, second_bound: Int, expected: Int) -> Int {
    if upper_bound_int(first_bound, second_bound) == expected {
        return 0
    }
    return 1
}

pub fn expect_int_quotient_or_zero(value: Int, divisor: Int, expected: Int) -> Int {
    if quotient_or_zero_int(value, divisor) == expected {
        return 0
    }
    return 1
}

pub fn expect_int_remainder_or_zero(value: Int, divisor: Int, expected: Int) -> Int {
    if remainder_or_zero_int(value, divisor) == expected {
        return 0
    }
    return 1
}

pub fn expect_int_has_remainder(value: Int, divisor: Int) -> Int {
    if has_remainder_int(value, divisor) {
        return 0
    }
    return 1
}

pub fn expect_int_factor_of(factor: Int, value: Int) -> Int {
    if is_factor_of_int(factor, value) {
        return 0
    }
    return 1
}

pub fn merge_statuses[N](statuses: [Int; N]) -> Int {
    var total = 0
    for status in statuses {
        total = total + status
    }
    return total
}

pub fn expect_int_between(actual: Int, low: Int, high: Int) -> Int {
    if in_range_int(actual, low, high) {
        return 0
    }
    return 1
}

pub fn expect_int_exclusive_between(actual: Int, low: Int, high: Int) -> Int {
    if in_exclusive_range_int(actual, low, high) {
        return 0
    }
    return 1
}

pub fn expect_int_outside(actual: Int, low: Int, high: Int) -> Int {
    if is_outside_range_int(actual, low, high) {
        return 0
    }
    return 1
}

pub fn expect_int_between_bounds(actual: Int, first_bound: Int, second_bound: Int) -> Int {
    if in_bounds_int(actual, first_bound, second_bound) {
        return 0
    }
    return 1
}

pub fn expect_int_exclusive_between_bounds(actual: Int, first_bound: Int, second_bound: Int) -> Int {
    if in_exclusive_bounds_int(actual, first_bound, second_bound) {
        return 0
    }
    return 1
}

pub fn expect_int_outside_bounds(actual: Int, first_bound: Int, second_bound: Int) -> Int {
    if is_outside_bounds_int(actual, first_bound, second_bound) {
        return 0
    }
    return 1
}

pub fn expect_int_clamped(actual: Int, low: Int, high: Int, expected: Int) -> Int {
    if clamp_int(actual, low, high) == expected {
        return 0
    }
    return 1
}

pub fn expect_int_clamp_min(actual: Int, low: Int, expected: Int) -> Int {
    if clamp_min_int(actual, low) == expected {
        return 0
    }
    return 1
}

pub fn expect_int_clamp_max(actual: Int, high: Int, expected: Int) -> Int {
    if clamp_max_int(actual, high) == expected {
        return 0
    }
    return 1
}

pub fn expect_int_clamped_bounds(actual: Int, first_bound: Int, second_bound: Int, expected: Int) -> Int {
    if clamp_bounds_int(actual, first_bound, second_bound) == expected {
        return 0
    }
    return 1
}

pub fn expect_int_distance_to_range(actual: Int, low: Int, high: Int, expected: Int) -> Int {
    if distance_to_range_int(actual, low, high) == expected {
        return 0
    }
    return 1
}

pub fn expect_int_distance_to_bounds(actual: Int, first_bound: Int, second_bound: Int, expected: Int) -> Int {
    if distance_to_bounds_int(actual, first_bound, second_bound) == expected {
        return 0
    }
    return 1
}

pub fn expect_int_even(actual: Int) -> Int {
    if is_even_int(actual) {
        return 0
    }
    return 1
}

pub fn expect_int_odd(actual: Int) -> Int {
    if is_odd_int(actual) {
        return 0
    }
    return 1
}

pub fn expect_int_divisible_by(actual: Int, divisor: Int) -> Int {
    if is_divisible_by_int(actual, divisor) {
        return 0
    }
    return 1
}

pub fn expect_int_within(actual: Int, target: Int, tolerance: Int) -> Int {
    if is_within_int(actual, target, tolerance) {
        return 0
    }
    return 1
}

pub fn expect_int_not_within(actual: Int, target: Int, tolerance: Int) -> Int {
    if is_not_within_int(actual, target, tolerance) {
        return 0
    }
    return 1
}

pub fn expect_int_positive(actual: Int) -> Int {
    if is_positive_int(actual) {
        return 0
    }
    return 1
}

pub fn expect_int_negative(actual: Int) -> Int {
    if is_negative_int(actual) {
        return 0
    }
    return 1
}

pub fn expect_int_nonnegative(actual: Int) -> Int {
    if is_nonnegative_int(actual) {
        return 0
    }
    return 1
}

pub fn expect_int_nonpositive(actual: Int) -> Int {
    if is_nonpositive_int(actual) {
        return 0
    }
    return 1
}

pub fn expect_bool_implies(left: Bool, right: Bool) -> Int {
    if implies_bool(left, right) {
        return 0
    }
    return 1
}

pub fn expect_option_some[T](value: Option[T], expected: T) -> Int {
    return match value {
        Option.Some(inner) => if inner == expected { 0 } else { 1 },
        Option.None => 1,
    }
}

pub fn expect_option_none[T](value: Option[T]) -> Int {
    return match value {
        Option.Some(_) => 1,
        Option.None => 0,
    }
}

pub fn expect_option_or[T](value: Option[T], fallback: Option[T], expected: T) -> Int {
    return match or_option(value, fallback) {
        Option.Some(inner) => if inner == expected { 0 } else { 1 },
        Option.None => 1,
    }
}

pub fn expect_result_ok[T, E](value: Result[T, E], expected: T) -> Int {
    return match to_option(value) {
        Option.Some(inner) => if inner == expected { 0 } else { 1 },
        Option.None => 1,
    }
}

pub fn expect_result_err[T, E](value: Result[T, E], expected_error: E) -> Int {
    return match error_to_option(value) {
        Option.Some(error) => if error == expected_error { 0 } else { 1 },
        Option.None => 1,
    }
}

pub fn expect_result_or[T, E](value: Result[T, E], fallback: Result[T, E], expected: T) -> Int {
    return match or_result(value, fallback) {
        Result.Ok(inner) => if inner == expected { 0 } else { 1 },
        Result.Err(_) => 1,
    }
}

pub fn expect_result_error[T, E](value: Result[T, E], fallback_error: E, expected_error: E) -> Int {
    if error_or(value, fallback_error) == expected_error {
        return 0
    }
    return 1
}

pub fn expect_result_to_option_some[T, E](value: Result[T, E], expected: T) -> Int {
    return match to_option(value) {
        Option.Some(inner) => if inner == expected { 0 } else { 1 },
        Option.None => 1,
    }
}

pub fn expect_result_to_option_none[T, E](value: Result[T, E]) -> Int {
    return match to_option(value) {
        Option.Some(_) => 1,
        Option.None => 0,
    }
}

pub fn expect_result_error_some[T, E](value: Result[T, E], expected_error: E) -> Int {
    return match error_to_option(value) {
        Option.Some(error) => if error == expected_error { 0 } else { 1 },
        Option.None => 1,
    }
}

pub fn expect_result_error_none[T, E](value: Result[T, E]) -> Int {
    return match error_to_option(value) {
        Option.Some(_) => 1,
        Option.None => 0,
    }
}

pub fn expect_option_ok_or[T, E](value: Option[T], error: E, expected: T) -> Int {
    return match ok_or(value, error) {
        Result.Ok(inner) => if inner == expected { 0 } else { 1 },
        Result.Err(_) => 1,
    }
}

pub fn expect_option_ok_or_err[T, E](value: Option[T], error: E) -> Int {
    return match ok_or(value, error) {
        Result.Ok(_) => 1,
        Result.Err(actual_error) => if actual_error == error { 0 } else { 1 },
    }
}
