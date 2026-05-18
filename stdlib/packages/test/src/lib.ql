package std.test

use std.core.abs_diff_int as abs_diff_int
use std.core.abs_int as abs_int
use std.core.and_bool as and_bool
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
use std.core.is_within_int as is_within_int
use std.core.lower_bound_int as lower_bound_int
use std.core.max_int as max_int
use std.core.min_int as min_int
use std.core.not_bool as not_bool
use std.core.or_bool as or_bool
use std.core.quotient_or_zero_int as quotient_or_zero_int
use std.core.range_span_int as range_span_int
use std.core.remainder_or_zero_int as remainder_or_zero_int
use std.core.sign_int as sign_int
use std.core.upper_bound_int as upper_bound_int
use std.core.xor_bool as xor_bool
use std.array.reverse_array as reverse_array
use std.option.Option as Option
use std.result.Result as Result

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
    return expect_eq(values[0], expected)
}

pub fn expect_array_last[T, N](values: [T; N], expected: T) -> Int {
    var last = values[0]
    for value in values {
        last = value
    }
    return expect_eq(last, expected)
}

pub fn expect_array_at[T, N](values: [T; N], index: Int, fallback: T, expected: T) -> Int {
    var current_index = 0
    for value in values {
        if current_index == index {
            return expect_eq(value, expected)
        };
        current_index = current_index + 1
    }
    return expect_eq(fallback, expected)
}

pub fn expect_array_contains[T, N](values: [T; N], needle: T, expected: Bool) -> Int {
    for value in values {
        if value == needle {
            return expect_eq(true, expected)
        }
    }
    return expect_eq(false, expected)
}

pub fn expect_array_count[T, N](values: [T; N], needle: T, expected: Int) -> Int {
    var count = 0
    for value in values {
        if value == needle {
            count = count + 1
        }
    }
    return expect_eq(count, expected)
}

pub fn expect_array_eq[T, N](actual: [T; N], expected: [T; N]) -> Int {
    var status = 0
    var index = 0
    for value in actual {
        status = status + expect_eq(value, expected[index]);
        index = index + 1
    }
    return status
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
    var total = 0
    for value in values {
        total = total + value
    }
    return expect_eq(total, expected)
}

pub fn expect_int_array_product[N](values: [Int; N], expected: Int) -> Int {
    var total = 1
    for value in values {
        total = total * value
    }
    return expect_eq(total, expected)
}

pub fn expect_int_array_average[N](values: [Int; N], expected: Int) -> Int {
    if N == 0 {
        return expect_eq(0, expected)
    }
    var total = 0
    for value in values {
        total = total + value
    }
    return expect_eq(total / N, expected)
}

pub fn expect_int_array_max[N](values: [Int; N], expected: Int) -> Int {
    var selected = values[0]
    for value in values {
        if value > selected {
            selected = value
        }
    }
    return expect_eq(selected, expected)
}

pub fn expect_int_array_min[N](values: [Int; N], expected: Int) -> Int {
    var selected = values[0]
    for value in values {
        if value < selected {
            selected = value
        }
    }
    return expect_eq(selected, expected)
}

pub fn expect_int_array_ascending[N](values: [Int; N]) -> Int {
    var index = 0
    var previous = 0
    for value in values {
        if index > 0 && value < previous {
            return 1
        };
        previous = value;
        index = index + 1
    }
    return 0
}

pub fn expect_int_array_strictly_ascending[N](values: [Int; N]) -> Int {
    var index = 0
    var previous = 0
    for value in values {
        if index > 0 && value <= previous {
            return 1
        };
        previous = value;
        index = index + 1
    }
    return 0
}

pub fn expect_int_array_descending[N](values: [Int; N]) -> Int {
    var index = 0
    var previous = 0
    for value in values {
        if index > 0 && value > previous {
            return 1
        };
        previous = value;
        index = index + 1
    }
    return 0
}

pub fn expect_int_array_strictly_descending[N](values: [Int; N]) -> Int {
    var index = 0
    var previous = 0
    for value in values {
        if index > 0 && value >= previous {
            return 1
        };
        previous = value;
        index = index + 1
    }
    return 0
}

pub fn expect_bool_array_all[N](values: [Bool; N], expected: Bool) -> Int {
    for value in values {
        if !value {
            return expect_eq(false, expected)
        }
    }
    return expect_eq(true, expected)
}

pub fn expect_bool_array_any[N](values: [Bool; N], expected: Bool) -> Int {
    for value in values {
        if value {
            return expect_eq(true, expected)
        }
    }
    return expect_eq(false, expected)
}

pub fn expect_bool_array_none[N](values: [Bool; N], expected: Bool) -> Int {
    for value in values {
        if value {
            return expect_eq(false, expected)
        }
    }
    return expect_eq(true, expected)
}

pub fn expect_array_reverse[T, N](values: [T; N], expected: [T; N]) -> Int {
    return expect_array_eq(reverse_array(values), expected)
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
        Option.Some(inner) => expect_eq(inner, expected),
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
    return match value {
        Option.Some(inner) => expect_eq(inner, expected),
        Option.None => expect_option_some(fallback, expected),
    }
}

pub fn expect_result_ok[T, E](value: Result[T, E], expected: T) -> Int {
    return match value {
        Result.Ok(inner) => expect_eq(inner, expected),
        Result.Err(_) => 1,
    }
}

pub fn expect_result_err[T, E](value: Result[T, E], expected_error: E) -> Int {
    return match value {
        Result.Ok(_) => 1,
        Result.Err(error) => expect_eq(error, expected_error),
    }
}

pub fn expect_result_or[T, E](value: Result[T, E], fallback: Result[T, E], expected: T) -> Int {
    return match value {
        Result.Ok(inner) => expect_eq(inner, expected),
        Result.Err(_) => expect_result_ok(fallback, expected),
    }
}

pub fn expect_result_error[T, E](value: Result[T, E], fallback_error: E, expected_error: E) -> Int {
    return match value {
        Result.Ok(_) => expect_eq(fallback_error, expected_error),
        Result.Err(error) => expect_eq(error, expected_error),
    }
}

pub fn expect_result_to_option_some[T, E](value: Result[T, E], expected: T) -> Int {
    return match value {
        Result.Ok(inner) => expect_eq(inner, expected),
        Result.Err(_) => 1,
    }
}

pub fn expect_result_to_option_none[T, E](value: Result[T, E]) -> Int {
    return match value {
        Result.Ok(_) => 1,
        Result.Err(_) => 0,
    }
}

pub fn expect_result_error_some[T, E](value: Result[T, E], expected_error: E) -> Int {
    return match value {
        Result.Ok(_) => 1,
        Result.Err(error) => expect_eq(error, expected_error),
    }
}

pub fn expect_result_error_none[T, E](value: Result[T, E]) -> Int {
    return match value {
        Result.Ok(_) => 0,
        Result.Err(_) => 1,
    }
}

pub fn expect_option_ok_or[T, E](value: Option[T], error: E, expected: T) -> Int {
    return match value {
        Option.Some(inner) => expect_eq(inner, expected),
        Option.None => 1,
    }
}

pub fn expect_option_ok_or_err[T, E](value: Option[T], error: E) -> Int {
    return match value {
        Option.Some(_) => 1,
        Option.None => 0,
    }
}
