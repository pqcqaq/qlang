use std.array.all_bool_array as all_bool_array
use std.array.any_bool_array as any_bool_array
use std.array.at_array_or as at_array_or
use std.array.average_int_array as average_int_array
use std.array.contains_array as contains_array
use std.array.count_array as count_array
use std.array.first_array as first_array
use std.array.last_array as last_array
use std.array.len_array as len_array
use std.array.max_int_array as max_int_array
use std.array.min_int_array as min_int_array
use std.array.none_bool_array as none_bool_array
use std.array.product_int_array as product_int_array
use std.array.repeat_array as repeat_array
use std.array.reverse_array as reverse_array
use std.array.sum_int_array as sum_int_array

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

fn sum6(first: Int, second: Int, third: Int, fourth: Int, fifth: Int, sixth: Int) -> Int {
    return first + second + third + fourth + fifth + sixth
}

fn front_back_status() -> Int {
    let int_status = sum6(check_int(first_array([2, 3, 4]), 2), check_int(first_array([2, 3, 4, 5]), 2), check_int(first_array([2, 3, 4, 5, 6]), 2), check_int(last_array([2, 3, 4]), 4), check_int(last_array([2, 3, 4, 5]), 5), check_int(last_array([2, 3, 4, 5, 6]), 6))
    let bool_status = sum6(check_bool(first_array([true, false, false]), true), check_bool(first_array([false, true, true, true]), false), check_bool(first_array([true, false, true, false, true]), true), check_bool(last_array([true, false, true]), true), check_bool(last_array([true, true, false, false]), false), check_bool(last_array([false, true, false, true, false]), false))
    return int_status + bool_status
}

fn at_status() -> Int {
    let int_status = sum6(check_int(at_array_or([2, 3, 4], 1, 99), 3), check_int(at_array_or([2, 3, 4, 5], 3, 99), 5), check_int(at_array_or([2, 3, 4, 5, 6], 4, 99), 6), check_int(at_array_or([2, 3, 4], 5, 99), 99), check_int(at_array_or([2, 3, 4, 5], 0, 99), 2), check_int(at_array_or([2, 3, 4, 5, 6], 2, 99), 4))
    let bool_status = sum6(check_bool(at_array_or([true, false, true], 1, true), false), check_bool(at_array_or([false, true, true, false], 3, true), false), check_bool(at_array_or([true, false, true, false, true], 4, false), true), check_bool(at_array_or([true, false, true], 5, false), false), check_bool(at_array_or([false, true, true, false], 0, true), false), check_bool(at_array_or([true, false, true, false, true], 2, false), true))
    return int_status + bool_status
}

fn len_reverse_status() -> Int {
    let len_status = sum6(check_int(len_array([2, 3, 4]), 3), check_int(len_array([2, 3, 4, 5]), 4), check_int(len_array([true, false, true, false, true]), 5), check_int(len_array(["red", "blue", "green"]), 3), 0, 0)
    let reversed_int: [Int; 4] = reverse_array([2, 3, 4, 5])
    let reversed_bool: [Bool; 5] = reverse_array([true, false, true, false, false])
    let reverse_status = sum6(check_int(first_array(reversed_int), 5), check_int(last_array(reversed_int), 2), check_bool(first_array(reversed_bool), false), check_bool(last_array(reversed_bool), true), 0, 0)
    let repeated_int: [Int; 4] = repeat_array(7)
    let repeated_bool: [Bool; 5] = repeat_array(false)
    let repeat_status = sum6(check_int(first_array(repeated_int), 7), check_int(last_array(repeated_int), 7), check_int(count_array(repeated_int, 7), 4), check_bool(first_array(repeated_bool), false), check_bool(last_array(repeated_bool), false), check_int(count_array(repeated_bool, false), 5))
    return len_status + reverse_status + repeat_status
}

fn contains_count_status() -> Int {
    let contains_status = sum6(check_bool(contains_array([2, 3, 4], 3), true), check_bool(contains_array([2, 3, 4, 5], 9), false), check_bool(contains_array([true, false, true, false, true], false), true), check_bool(contains_array(["red", "blue", "green"], "blue"), true), check_bool(contains_array(["a", "b", "c", "d"], "z"), false), 0)
    let count_status = sum6(check_int(count_array([2, 3, 2], 2), 2), check_int(count_array([2, 3, 2, 2], 2), 3), check_int(count_array([true, false, true, false, true], true), 3), check_int(count_array([1, 2, 3, 4, 5], 9), 0), check_int(count_array(["same", "other", "same"], "same"), 2), 0)
    return contains_status + count_status
}

fn numeric_status() -> Int {
    let aggregate_status = sum6(check_int(sum_int_array([2, 3, 4]), 9), check_int(sum_int_array([2, 3, 4, 5]), 14), check_int(sum_int_array([2, 3, 4, 5, 6]), 20), check_int(product_int_array([2, 3, 4]), 24), check_int(product_int_array([2, 3, 4, 5]), 120), check_int(product_int_array([2, 3, 4, 5, 6]), 720))
    let average_status = sum6(check_int(average_int_array([5, 8]), 6), check_int(average_int_array([3, 6, 9]), 6), check_int(average_int_array([2, 4, 6, 8]), 5), check_int(average_int_array([2, 4, 6, 8, 10]), 6), 0, 0)
    let extrema_status = sum6(check_int(max_int_array([3, 9, 5]), 9), check_int(max_int_array([3, 9, 5, 7]), 9), check_int(max_int_array([3, 9, 5, 7, 11]), 11), check_int(min_int_array([3, 9, 5]), 3), check_int(min_int_array([3, 9, 5, 7]), 3), check_int(min_int_array([3, 9, 5, 7, 1]), 1))
    return aggregate_status + average_status + extrema_status
}

fn bool_status() -> Int {
    let truthy_status = sum6(check_bool(all_bool_array([true, true, true]), true), check_bool(all_bool_array([true, true, true, false]), false), check_bool(any_bool_array([false, false, true]), true), check_bool(any_bool_array([false, false, false, false]), false), check_bool(any_bool_array([false, false, false, false, true]), true), 0)
    let none_status = sum6(check_bool(none_bool_array([false, false, false]), true), check_bool(none_bool_array([false, false, true, false]), false), check_bool(none_bool_array([false, false, false, false, false]), true), 0, 0, 0)
    return truthy_status + none_status
}

fn main() -> Int {
    return front_back_status() + at_status() + len_reverse_status() + contains_count_status() + numeric_status() + bool_status()
}
