package std.array

use std.core.all_bools as core_all_bools
use std.core.any_bools as core_any_bools
use std.core.average_ints as core_average_ints
use std.core.max_ints as core_max_ints
use std.core.min_ints as core_min_ints
use std.core.none_bools as core_none_bools
use std.core.product_ints as core_product_ints
use std.core.sum_ints as core_sum_ints

pub fn first_array[T, N](values: [T; N]) -> T {
    return values[0]
}

pub fn last_array[T, N](values: [T; N]) -> T {
    var last = values[0]
    for value in values {
        last = value
    }
    return last
}

pub fn at_array_or[T, N](values: [T; N], index: Int, fallback: T) -> T {
    var current_index = 0
    for value in values {
        if current_index == index {
            return value
        };
        current_index = current_index + 1
    }
    return fallback
}

pub fn contains_array[T, N](values: [T; N], needle: T) -> Bool {
    for value in values {
        if value == needle {
            return true
        }
    }
    return false
}

pub fn count_array[T, N](values: [T; N], needle: T) -> Int {
    var count = 0
    for value in values {
        if value == needle {
            count = count + 1
        }
    }
    return count
}

pub fn count_mismatches_array[T, N](actual: [T; N], expected: [T; N]) -> Int {
    var count = 0
    var index = 0
    for value in actual {
        if value != expected[index] {
            count = count + 1
        };
        index = index + 1
    }
    return count
}

pub fn len_array[T, N](values: [T; N]) -> Int {
    return N
}

pub fn reverse_array[T, N](values: [T; N]) -> [T; N] {
    var result = values
    var index = 0
    for value in values {
        result[index] = values[N - index - 1];
        index = index + 1
    }
    return result
}

pub fn repeat_array[T, N](value: T) -> [T; N] {
    return [value; N]
}

pub fn sum_int_array[N](values: [Int; N]) -> Int {
    return core_sum_ints(values)
}

pub fn product_int_array[N](values: [Int; N]) -> Int {
    return core_product_ints(values)
}

pub fn average_int_array[N](values: [Int; N]) -> Int {
    return core_average_ints(values)
}

pub fn max_int_array[N](values: [Int; N]) -> Int {
    return core_max_ints(values)
}

pub fn min_int_array[N](values: [Int; N]) -> Int {
    return core_min_ints(values)
}

pub fn all_bool_array[N](values: [Bool; N]) -> Bool {
    return core_all_bools(values)
}

pub fn any_bool_array[N](values: [Bool; N]) -> Bool {
    return core_any_bools(values)
}

pub fn none_bool_array[N](values: [Bool; N]) -> Bool {
    return core_none_bools(values)
}
