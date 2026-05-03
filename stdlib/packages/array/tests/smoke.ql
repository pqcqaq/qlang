use std.array.all3_bool_array as all3_bool_array
use std.array.all4_bool_array as all4_bool_array
use std.array.all5_bool_array as all5_bool_array
use std.array.all_bool_array as all_bool_array
use std.array.at_array_or as at_array_or
use std.array.at3_array_or as at3_array_or
use std.array.at4_array_or as at4_array_or
use std.array.at5_array_or as at5_array_or
use std.array.any3_bool_array as any3_bool_array
use std.array.any4_bool_array as any4_bool_array
use std.array.any5_bool_array as any5_bool_array
use std.array.any_bool_array as any_bool_array
use std.array.contains_array as contains_array
use std.array.contains3_array as contains3_array
use std.array.contains4_array as contains4_array
use std.array.contains5_array as contains5_array
use std.array.count_array as count_array
use std.array.count3_array as count3_array
use std.array.count4_array as count4_array
use std.array.count5_array as count5_array
use std.array.first_array as first_array
use std.array.first3_array as first3_array
use std.array.first4_array as first4_array
use std.array.first5_array as first5_array
use std.array.last_array as last_array
use std.array.last3_array as last3_array
use std.array.last4_array as last4_array
use std.array.last5_array as last5_array
use std.array.max3_int_array as max3_int_array
use std.array.max4_int_array as max4_int_array
use std.array.max5_int_array as max5_int_array
use std.array.max_int_array as max_int_array
use std.array.min3_int_array as min3_int_array
use std.array.min4_int_array as min4_int_array
use std.array.min5_int_array as min5_int_array
use std.array.min_int_array as min_int_array
use std.array.none3_bool_array as none3_bool_array
use std.array.none4_bool_array as none4_bool_array
use std.array.none5_bool_array as none5_bool_array
use std.array.none_bool_array as none_bool_array
use std.array.product3_int_array as product3_int_array
use std.array.product4_int_array as product4_int_array
use std.array.product5_int_array as product5_int_array
use std.array.product_int_array as product_int_array
use std.array.repeat3_array as repeat3_array
use std.array.repeat4_array as repeat4_array
use std.array.repeat5_array as repeat5_array
use std.array.reverse3_array as reverse3_array
use std.array.reverse4_array as reverse4_array
use std.array.reverse5_array as reverse5_array
use std.array.sum3_int_array as sum3_int_array
use std.array.sum4_int_array as sum4_int_array
use std.array.sum5_int_array as sum5_int_array
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

fn main() -> Int {
    let generic_int_status = sum6(check_int(first_array([2, 3, 4]), 2), check_int(first_array([2, 3, 4, 5]), 2), check_int(first_array([2, 3, 4, 5, 6]), 2), check_int(last_array([2, 3, 4]), 4), check_int(last_array([2, 3, 4, 5]), 5), check_int(last_array([2, 3, 4, 5, 6]), 6))
    let generic_bool_status = sum6(check_bool(first_array([true, false, false]), true), check_bool(first_array([false, true, true, true]), false), check_bool(first_array([true, false, true, false, true]), true), check_bool(last_array([true, false, true]), true), check_bool(last_array([true, true, false, false]), false), check_bool(last_array([false, true, false, true, false]), false))
    let compatibility_status = sum6(check_int(first3_array([2, 3, 4]), 2), check_int(first4_array([2, 3, 4, 5]), 2), check_int(first5_array([2, 3, 4, 5, 6]), 2), check_int(last3_array([2, 3, 4]), 4), check_int(last4_array([2, 3, 4, 5]), 5), check_int(last5_array([2, 3, 4, 5, 6]), 6))
    let canonical_at_int_status = sum6(check_int(at_array_or([2, 3, 4], 1, 99), 3), check_int(at_array_or([2, 3, 4, 5], 3, 99), 5), check_int(at_array_or([2, 3, 4, 5, 6], 4, 99), 6), check_int(at_array_or([2, 3, 4], 5, 99), 99), check_int(at_array_or([2, 3, 4, 5], 0, 99), 2), check_int(at_array_or([2, 3, 4, 5, 6], 2, 99), 4))
    let canonical_at_bool_status = sum6(check_bool(at_array_or([true, false, true], 1, true), false), check_bool(at_array_or([false, true, true, false], 3, true), false), check_bool(at_array_or([true, false, true, false, true], 4, false), true), check_bool(at_array_or([true, false, true], 5, false), false), check_bool(at_array_or([false, true, true, false], 0, true), false), check_bool(at_array_or([true, false, true, false, true], 2, false), true))
    let generic_at_int_status = sum6(check_int(at3_array_or([2, 3, 4], 1, 99), 3), check_int(at4_array_or([2, 3, 4, 5], 3, 99), 5), check_int(at5_array_or([2, 3, 4, 5, 6], 4, 99), 6), check_int(at3_array_or([2, 3, 4], 5, 99), 99), check_int(at4_array_or([2, 3, 4, 5], 0, 99), 2), check_int(at5_array_or([2, 3, 4, 5, 6], 2, 99), 4))
    let generic_at_bool_status = sum6(check_bool(at3_array_or([true, false, true], 1, true), false), check_bool(at4_array_or([false, true, true, false], 3, true), false), check_bool(at5_array_or([true, false, true, false, true], 4, false), true), check_bool(at3_array_or([true, false, true], 5, false), false), check_bool(at4_array_or([false, true, true, false], 0, true), false), check_bool(at5_array_or([true, false, true, false, true], 2, false), true))
    let reversed3_int: [Int; 3] = reverse3_array([2, 3, 4])
    let reversed4_int: [Int; 4] = reverse4_array([2, 3, 4, 5])
    let reversed5_int: [Int; 5] = reverse5_array([2, 3, 4, 5, 6])
    let repeated3_int: [Int; 3] = repeat3_array(4)
    let repeated4_int: [Int; 4] = repeat4_array(3)
    let repeated5_int: [Int; 5] = repeat5_array(2)
    let repeated3_bool: [Bool; 3] = repeat3_array(true)
    let repeated4_bool: [Bool; 4] = repeat4_array(false)
    let repeated5_bool: [Bool; 5] = repeat5_array(false)
    let generic_reverse_int_status = sum6(check_int(first3_array(reversed3_int), 4), check_int(last3_array(reversed3_int), 2), check_int(first4_array(reversed4_int), 5), check_int(last4_array(reversed4_int), 2), check_int(first5_array(reversed5_int), 6), check_int(last5_array(reversed5_int), 2))
    let generic_repeat_status = sum6(check_int(sum3_int_array(repeated3_int), 12), check_int(sum4_int_array(repeated4_int), 12), check_int(sum5_int_array(repeated5_int), 10), check_bool(all3_bool_array(repeated3_bool), true), check_bool(any4_bool_array(repeated4_bool), false), check_bool(none5_bool_array(repeated5_bool), true))
    let canonical_contains_status = sum6(check_bool(contains_array([2, 3, 4], 3), true), check_bool(contains_array([2, 3, 4, 5], 9), false), check_bool(contains_array([true, false, true, false, true], false), true), check_bool(contains_array(["red", "blue", "green"], "blue"), true), check_bool(contains_array(["a", "b", "c", "d"], "z"), false), 0)
    let canonical_count_status = sum6(check_int(count_array([2, 3, 2], 2), 2), check_int(count_array([2, 3, 2, 2], 2), 3), check_int(count_array([true, false, true, false, true], true), 3), check_int(count_array([1, 2, 3, 4, 5], 9), 0), check_int(count_array(["same", "other", "same"], "same"), 2), 0)
    let generic_contains_status = sum6(check_bool(contains3_array([2, 3, 4], 3), true), check_bool(contains4_array([2, 3, 4, 5], 9), false), check_bool(contains5_array([true, false, true, false, true], false), true), check_bool(contains3_array(["red", "blue", "green"], "blue"), true), check_bool(contains4_array(["a", "b", "c", "d"], "z"), false), 0)
    let generic_count_status = sum6(check_int(count3_array([2, 3, 2], 2), 2), check_int(count4_array([2, 3, 2, 2], 2), 3), check_int(count5_array([true, false, true, false, true], true), 3), check_int(count5_array([1, 2, 3, 4, 5], 9), 0), check_int(count3_array(["same", "other", "same"], "same"), 2), 0)
    let numeric_status = sum6(check_int(sum_int_array([2, 3, 4]), 9), check_int(sum_int_array([2, 3, 4, 5]), 14), check_int(sum_int_array([2, 3, 4, 5, 6]), 20), check_int(product_int_array([2, 3, 4]), 24), check_int(product_int_array([2, 3, 4, 5]), 120), check_int(product_int_array([2, 3, 4, 5, 6]), 720))
    let numeric_compatibility_status = sum6(check_int(sum3_int_array([2, 3, 4]), 9), check_int(sum4_int_array([2, 3, 4, 5]), 14), check_int(sum5_int_array([2, 3, 4, 5, 6]), 20), check_int(product3_int_array([2, 3, 4]), 24), check_int(product4_int_array([2, 3, 4, 5]), 120), check_int(product5_int_array([2, 3, 4, 5, 6]), 720))
    let extrema_status = sum6(check_int(max_int_array([3, 9, 5]), 9), check_int(max_int_array([3, 9, 5, 7]), 9), check_int(max_int_array([3, 9, 5, 7, 11]), 11), check_int(min_int_array([3, 9, 5]), 3), check_int(min_int_array([3, 9, 5, 7]), 3), check_int(min_int_array([3, 9, 5, 7, 1]), 1))
    let extrema_compatibility_status = sum6(check_int(max3_int_array([3, 9, 5]), 9), check_int(max4_int_array([3, 9, 5, 7]), 9), check_int(max5_int_array([3, 9, 5, 7, 11]), 11), check_int(min3_int_array([3, 9, 5]), 3), check_int(min4_int_array([3, 9, 5, 7]), 3), check_int(min5_int_array([3, 9, 5, 7, 1]), 1))
    let bool_all_status = sum6(check_bool(all_bool_array([true, true, true]), true), check_bool(all_bool_array([true, true, true, false]), false), check_bool(all_bool_array([true, true, true, true, true]), true), check_bool(any_bool_array([false, false, true]), true), check_bool(any_bool_array([false, false, false, false]), false), check_bool(any_bool_array([false, false, false, false, true]), true))
    let bool_all_compatibility_status = sum6(check_bool(all3_bool_array([true, true, true]), true), check_bool(all4_bool_array([true, true, true, false]), false), check_bool(all5_bool_array([true, true, true, true, true]), true), check_bool(any3_bool_array([false, false, true]), true), check_bool(any4_bool_array([false, false, false, false]), false), check_bool(any5_bool_array([false, false, false, false, true]), true))
    let bool_none_status = sum6(check_bool(none_bool_array([false, false, false]), true), check_bool(none_bool_array([false, false, true, false]), false), check_bool(none_bool_array([false, false, false, false, false]), true), 0, 0, 0)
    let bool_none_compatibility_status = sum6(check_bool(none3_bool_array([false, false, false]), true), check_bool(none4_bool_array([false, false, true, false]), false), check_bool(none5_bool_array([false, false, false, false, false]), true), 0, 0, 0)
    return generic_int_status + generic_bool_status + compatibility_status + canonical_at_int_status + canonical_at_bool_status + generic_at_int_status + generic_at_bool_status + generic_reverse_int_status + generic_repeat_status + canonical_contains_status + canonical_count_status + generic_contains_status + generic_count_status + numeric_status + numeric_compatibility_status + extrema_status + extrema_compatibility_status + bool_all_status + bool_all_compatibility_status + bool_none_status + bool_none_compatibility_status
}
