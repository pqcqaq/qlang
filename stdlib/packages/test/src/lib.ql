package std.test

use std.core.all3_bool as all3_bool
use std.core.all4_bool as all4_bool
use std.core.and_bool as and_bool
use std.core.any3_bool as any3_bool
use std.core.any4_bool as any4_bool
use std.core.average2_int as average2_int
use std.core.average3_int as average3_int
use std.core.clamp_bounds_int as clamp_bounds_int
use std.core.clamp_int as clamp_int
use std.core.compare_int as compare_int
use std.core.distance_to_bounds_int as distance_to_bounds_int
use std.core.distance_to_range_int as distance_to_range_int
use std.core.implies_bool as implies_bool
use std.core.in_bounds_int as in_bounds_int
use std.core.in_exclusive_bounds_int as in_exclusive_bounds_int
use std.core.in_exclusive_range_int as in_exclusive_range_int
use std.core.in_range_int as in_range_int
use std.core.has_remainder_int as has_remainder_int
use std.core.is_ascending_int as is_ascending_int
use std.core.is_descending_int as is_descending_int
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
use std.core.is_strictly_descending_int as is_strictly_descending_int
use std.core.is_strictly_ascending_int as is_strictly_ascending_int
use std.core.is_within_int as is_within_int
use std.core.max3_int as max3_int
use std.core.max4_int as max4_int
use std.core.max_int as max_int
use std.core.min3_int as min3_int
use std.core.min4_int as min4_int
use std.core.min_int as min_int
use std.core.none3_bool as none3_bool
use std.core.none4_bool as none4_bool
use std.core.not_bool as not_bool
use std.core.or_bool as or_bool
use std.core.product3_int as product3_int
use std.core.product4_int as product4_int
use std.core.quotient_or_zero_int as quotient_or_zero_int
use std.core.remainder_or_zero_int as remainder_or_zero_int
use std.core.sign_int as sign_int
use std.core.sum3_int as sum3_int
use std.core.sum4_int as sum4_int
use std.core.xor_bool as xor_bool

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

pub fn expect_bool_eq(actual: Bool, expected: Bool) -> Int {
    if actual == expected {
        return 0
    }
    return 1
}

pub fn expect_bool_ne(actual: Bool, unexpected: Bool) -> Int {
    if actual != unexpected {
        return 0
    }
    return 1
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

pub fn expect_bool_all3(first: Bool, second: Bool, third: Bool, expected: Bool) -> Int {
    if all3_bool(first, second, third) == expected {
        return 0
    }
    return 1
}

pub fn expect_bool_all4(first: Bool, second: Bool, third: Bool, fourth: Bool, expected: Bool) -> Int {
    if all4_bool(first, second, third, fourth) == expected {
        return 0
    }
    return 1
}

pub fn expect_bool_any3(first: Bool, second: Bool, third: Bool, expected: Bool) -> Int {
    if any3_bool(first, second, third) == expected {
        return 0
    }
    return 1
}

pub fn expect_bool_any4(first: Bool, second: Bool, third: Bool, fourth: Bool, expected: Bool) -> Int {
    if any4_bool(first, second, third, fourth) == expected {
        return 0
    }
    return 1
}

pub fn expect_bool_none3(first: Bool, second: Bool, third: Bool, expected: Bool) -> Int {
    if none3_bool(first, second, third) == expected {
        return 0
    }
    return 1
}

pub fn expect_bool_none4(first: Bool, second: Bool, third: Bool, fourth: Bool, expected: Bool) -> Int {
    if none4_bool(first, second, third, fourth) == expected {
        return 0
    }
    return 1
}

pub fn expect_int_eq(actual: Int, expected: Int) -> Int {
    if actual == expected {
        return 0
    }
    return 1
}

pub fn expect_int_ne(actual: Int, unexpected: Int) -> Int {
    if actual != unexpected {
        return 0
    }
    return 1
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

pub fn expect_int_max3(first: Int, second: Int, third: Int, expected: Int) -> Int {
    if max3_int(first, second, third) == expected {
        return 0
    }
    return 1
}

pub fn expect_int_min3(first: Int, second: Int, third: Int, expected: Int) -> Int {
    if min3_int(first, second, third) == expected {
        return 0
    }
    return 1
}

pub fn expect_int_max4(first: Int, second: Int, third: Int, fourth: Int, expected: Int) -> Int {
    if max4_int(first, second, third, fourth) == expected {
        return 0
    }
    return 1
}

pub fn expect_int_min4(first: Int, second: Int, third: Int, fourth: Int, expected: Int) -> Int {
    if min4_int(first, second, third, fourth) == expected {
        return 0
    }
    return 1
}

pub fn expect_int_sum3(first: Int, second: Int, third: Int, expected: Int) -> Int {
    if sum3_int(first, second, third) == expected {
        return 0
    }
    return 1
}

pub fn expect_int_sum4(first: Int, second: Int, third: Int, fourth: Int, expected: Int) -> Int {
    if sum4_int(first, second, third, fourth) == expected {
        return 0
    }
    return 1
}

pub fn expect_int_product3(first: Int, second: Int, third: Int, expected: Int) -> Int {
    if product3_int(first, second, third) == expected {
        return 0
    }
    return 1
}

pub fn expect_int_product4(first: Int, second: Int, third: Int, fourth: Int, expected: Int) -> Int {
    if product4_int(first, second, third, fourth) == expected {
        return 0
    }
    return 1
}

pub fn expect_int_average2(left: Int, right: Int, expected: Int) -> Int {
    if average2_int(left, right) == expected {
        return 0
    }
    return 1
}

pub fn expect_int_average3(first: Int, second: Int, third: Int, expected: Int) -> Int {
    if average3_int(first, second, third) == expected {
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

pub fn is_status_ok(status: Int) -> Bool {
    return status == 0
}

pub fn is_status_failed(status: Int) -> Bool {
    return status != 0
}

pub fn merge_status(left: Int, right: Int) -> Int {
    return left + right
}

pub fn merge_status3(first: Int, second: Int, third: Int) -> Int {
    return merge_status(merge_status(first, second), third)
}

pub fn merge_status4(first: Int, second: Int, third: Int, fourth: Int) -> Int {
    return merge_status(merge_status3(first, second, third), fourth)
}

pub fn merge_status5(first: Int, second: Int, third: Int, fourth: Int, fifth: Int) -> Int {
    return merge_status(merge_status4(first, second, third, fourth), fifth)
}

pub fn merge_status6(first: Int, second: Int, third: Int, fourth: Int, fifth: Int, sixth: Int) -> Int {
    return merge_status(merge_status5(first, second, third, fourth, fifth), sixth)
}

pub fn expect_status_ok(status: Int) -> Int {
    if is_status_ok(status) {
        return 0
    }
    return 1
}

pub fn expect_status_failed(status: Int) -> Int {
    if is_status_failed(status) {
        return 0
    }
    return 1
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

pub fn expect_int_ascending(first: Int, second: Int, third: Int) -> Int {
    if is_ascending_int(first, second, third) {
        return 0
    }
    return 1
}

pub fn expect_int_strictly_ascending(first: Int, second: Int, third: Int) -> Int {
    if is_strictly_ascending_int(first, second, third) {
        return 0
    }
    return 1
}

pub fn expect_int_descending(first: Int, second: Int, third: Int) -> Int {
    if is_descending_int(first, second, third) {
        return 0
    }
    return 1
}

pub fn expect_int_strictly_descending(first: Int, second: Int, third: Int) -> Int {
    if is_strictly_descending_int(first, second, third) {
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
