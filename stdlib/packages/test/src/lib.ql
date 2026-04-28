package std.test

use std.core.implies_bool as implies_bool
use std.core.in_exclusive_range_int as in_exclusive_range_int
use std.core.in_range_int as in_range_int
use std.core.is_ascending_int as is_ascending_int
use std.core.is_divisible_by_int as is_divisible_by_int
use std.core.is_even_int as is_even_int
use std.core.is_negative_int as is_negative_int
use std.core.is_nonnegative_int as is_nonnegative_int
use std.core.is_nonpositive_int as is_nonpositive_int
use std.core.is_odd_int as is_odd_int
use std.core.is_positive_int as is_positive_int
use std.core.is_strictly_ascending_int as is_strictly_ascending_int
use std.core.is_within_int as is_within_int
use std.core.not_bool as not_bool

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
    if not_bool(in_range_int(actual, low, high)) {
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
    if not_bool(is_within_int(actual, target, tolerance)) {
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
