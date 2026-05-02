package std.test

use std.array.all3_bool_array as array_all3_bool
use std.array.all4_bool_array as array_all4_bool
use std.array.all5_bool_array as array_all5_bool
use std.array.at3_array_or as array_at3_or
use std.array.at4_array_or as array_at4_or
use std.array.at5_array_or as array_at5_or
use std.array.any3_bool_array as array_any3_bool
use std.array.any4_bool_array as array_any4_bool
use std.array.any5_bool_array as array_any5_bool
use std.array.first3_array as array_first3
use std.array.first4_array as array_first4
use std.array.first5_array as array_first5
use std.array.last3_array as array_last3
use std.array.last4_array as array_last4
use std.array.last5_array as array_last5
use std.array.max3_int_array as array_max3_int
use std.array.max4_int_array as array_max4_int
use std.array.max5_int_array as array_max5_int
use std.array.min3_int_array as array_min3_int
use std.array.min4_int_array as array_min4_int
use std.array.min5_int_array as array_min5_int
use std.array.none3_bool_array as array_none3_bool
use std.array.none4_bool_array as array_none4_bool
use std.array.none5_bool_array as array_none5_bool
use std.array.product3_int_array as array_product3_int
use std.array.product4_int_array as array_product4_int
use std.array.product5_int_array as array_product5_int
use std.array.repeat3_array as array_repeat3
use std.array.repeat4_array as array_repeat4
use std.array.repeat5_array as array_repeat5
use std.array.reverse3_array as array_reverse3
use std.array.reverse4_array as array_reverse4
use std.array.reverse5_array as array_reverse5
use std.array.sum3_int_array as array_sum3_int
use std.array.sum4_int_array as array_sum4_int
use std.array.sum5_int_array as array_sum5_int
use std.core.abs_diff_int as abs_diff_int
use std.core.abs_int as abs_int
use std.core.all3_bool as all3_bool
use std.core.all4_bool as all4_bool
use std.core.all5_bool as all5_bool
use std.core.and_bool as and_bool
use std.core.any3_bool as any3_bool
use std.core.any4_bool as any4_bool
use std.core.any5_bool as any5_bool
use std.core.average2_int as average2_int
use std.core.average3_int as average3_int
use std.core.average4_int as average4_int
use std.core.average5_int as average5_int
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
use std.core.is_ascending_int as is_ascending_int
use std.core.is_descending_int as is_descending_int
use std.core.is_descending4_int as is_descending4_int
use std.core.is_descending5_int as is_descending5_int
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
use std.core.is_ascending4_int as is_ascending4_int
use std.core.is_ascending5_int as is_ascending5_int
use std.core.is_strictly_ascending4_int as is_strictly_ascending4_int
use std.core.is_strictly_ascending5_int as is_strictly_ascending5_int
use std.core.is_strictly_descending4_int as is_strictly_descending4_int
use std.core.is_strictly_descending5_int as is_strictly_descending5_int
use std.core.is_within_int as is_within_int
use std.core.lower_bound_int as lower_bound_int
use std.core.max3_int as max3_int
use std.core.max4_int as max4_int
use std.core.max5_int as max5_int
use std.core.max_int as max_int
use std.core.median3_int as median3_int
use std.core.min3_int as min3_int
use std.core.min4_int as min4_int
use std.core.min5_int as min5_int
use std.core.min_int as min_int
use std.core.none3_bool as none3_bool
use std.core.none4_bool as none4_bool
use std.core.none5_bool as none5_bool
use std.core.not_bool as not_bool
use std.core.or_bool as or_bool
use std.core.product3_int as product3_int
use std.core.product4_int as product4_int
use std.core.product5_int as product5_int
use std.core.quotient_or_zero_int as quotient_or_zero_int
use std.core.range_span_int as range_span_int
use std.core.remainder_or_zero_int as remainder_or_zero_int
use std.core.sign_int as sign_int
use std.core.sum3_int as sum3_int
use std.core.sum4_int as sum4_int
use std.core.sum5_int as sum5_int
use std.core.upper_bound_int as upper_bound_int
use std.core.xor_bool as xor_bool
use std.option.BoolOption as BoolOption
use std.option.Option as Option
use std.option.is_none_bool as option_is_none_bool
use std.option.is_none_int as option_is_none_int
use std.option.is_some_bool as option_is_some_bool
use std.option.is_some_int as option_is_some_int
use std.option.or_option_bool as option_or_bool
use std.option.or_option_int as option_or_int
use std.option.unwrap_or_bool as option_unwrap_or_bool
use std.option.unwrap_or_int as option_unwrap_or_int
use std.option.IntOption as IntOption
use std.result.BoolResult as BoolResult
use std.result.Result as Result
use std.result.error_to_option as result_error_to_option
use std.result.error_to_option_bool as result_error_to_option_bool
use std.result.error_to_option_int as result_error_to_option_int
use std.result.error_or_zero_bool as result_error_or_zero_bool
use std.result.error_or_zero_int as result_error_or_zero_int
use std.result.is_err_bool as result_is_err_bool
use std.result.is_err_int as result_is_err_int
use std.result.is_ok_bool as result_is_ok_bool
use std.result.is_ok_int as result_is_ok_int
use std.result.ok_or as result_ok_or
use std.result.ok_or_bool as result_ok_or_bool
use std.result.ok_or_int as result_ok_or_int
use std.result.or_result_bool as result_or_bool
use std.result.or_result_int as result_or_int
use std.result.to_option as result_to_option
use std.result.to_option_bool as result_to_option_bool
use std.result.to_option_int as result_to_option_int
use std.result.unwrap_result_or_bool as result_unwrap_or_bool
use std.result.unwrap_result_or_int as result_unwrap_or_int
use std.result.IntResult as IntResult

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

pub fn expect_bool_all5(first: Bool, second: Bool, third: Bool, fourth: Bool, fifth: Bool, expected: Bool) -> Int {
    if all5_bool(first, second, third, fourth, fifth) == expected {
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

pub fn expect_bool_any5(first: Bool, second: Bool, third: Bool, fourth: Bool, fifth: Bool, expected: Bool) -> Int {
    if any5_bool(first, second, third, fourth, fifth) == expected {
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

pub fn expect_bool_none5(first: Bool, second: Bool, third: Bool, fourth: Bool, fifth: Bool, expected: Bool) -> Int {
    if none5_bool(first, second, third, fourth, fifth) == expected {
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

pub fn expect_int_eq(actual: Int, expected: Int) -> Int {
    if actual == expected {
        return 0
    }
    return 1
}

pub fn expect_int_array_first3(values: [Int; 3], expected: Int) -> Int {
    return expect_int_eq(array_first3(values), expected)
}

pub fn expect_int_array_first4(values: [Int; 4], expected: Int) -> Int {
    return expect_int_eq(array_first4(values), expected)
}

pub fn expect_int_array_first5(values: [Int; 5], expected: Int) -> Int {
    return expect_int_eq(array_first5(values), expected)
}

pub fn expect_int_array_last3(values: [Int; 3], expected: Int) -> Int {
    return expect_int_eq(array_last3(values), expected)
}

pub fn expect_int_array_last4(values: [Int; 4], expected: Int) -> Int {
    return expect_int_eq(array_last4(values), expected)
}

pub fn expect_int_array_last5(values: [Int; 5], expected: Int) -> Int {
    return expect_int_eq(array_last5(values), expected)
}

pub fn expect_bool_array_first3(values: [Bool; 3], expected: Bool) -> Int {
    return expect_bool_eq(array_first3(values), expected)
}

pub fn expect_bool_array_first4(values: [Bool; 4], expected: Bool) -> Int {
    return expect_bool_eq(array_first4(values), expected)
}

pub fn expect_bool_array_first5(values: [Bool; 5], expected: Bool) -> Int {
    return expect_bool_eq(array_first5(values), expected)
}

pub fn expect_bool_array_last3(values: [Bool; 3], expected: Bool) -> Int {
    return expect_bool_eq(array_last3(values), expected)
}

pub fn expect_bool_array_last4(values: [Bool; 4], expected: Bool) -> Int {
    return expect_bool_eq(array_last4(values), expected)
}

pub fn expect_bool_array_last5(values: [Bool; 5], expected: Bool) -> Int {
    return expect_bool_eq(array_last5(values), expected)
}

pub fn expect_int_array_at3(values: [Int; 3], index: Int, fallback: Int, expected: Int) -> Int {
    return expect_int_eq(array_at3_or(values, index, fallback), expected)
}

pub fn expect_int_array_at4(values: [Int; 4], index: Int, fallback: Int, expected: Int) -> Int {
    return expect_int_eq(array_at4_or(values, index, fallback), expected)
}

pub fn expect_int_array_at5(values: [Int; 5], index: Int, fallback: Int, expected: Int) -> Int {
    return expect_int_eq(array_at5_or(values, index, fallback), expected)
}

pub fn expect_bool_array_at3(values: [Bool; 3], index: Int, fallback: Bool, expected: Bool) -> Int {
    return expect_bool_eq(array_at3_or(values, index, fallback), expected)
}

pub fn expect_bool_array_at4(values: [Bool; 4], index: Int, fallback: Bool, expected: Bool) -> Int {
    return expect_bool_eq(array_at4_or(values, index, fallback), expected)
}

pub fn expect_bool_array_at5(values: [Bool; 5], index: Int, fallback: Bool, expected: Bool) -> Int {
    return expect_bool_eq(array_at5_or(values, index, fallback), expected)
}

pub fn expect_int_array_reverse3(values: [Int; 3], expected_first: Int, expected_last: Int) -> Int {
    let reversed: [Int; 3] = array_reverse3(values)
    return expect_int_eq(array_first3(reversed), expected_first) + expect_int_eq(array_last3(reversed), expected_last)
}

pub fn expect_int_array_reverse4(values: [Int; 4], expected_first: Int, expected_last: Int) -> Int {
    let reversed: [Int; 4] = array_reverse4(values)
    return expect_int_eq(array_first4(reversed), expected_first) + expect_int_eq(array_last4(reversed), expected_last)
}

pub fn expect_int_array_reverse5(values: [Int; 5], expected_first: Int, expected_last: Int) -> Int {
    let reversed: [Int; 5] = array_reverse5(values)
    return expect_int_eq(array_first5(reversed), expected_first) + expect_int_eq(array_last5(reversed), expected_last)
}

pub fn expect_bool_array_reverse3(values: [Bool; 3], expected_first: Bool, expected_last: Bool) -> Int {
    let reversed: [Bool; 3] = array_reverse3(values)
    return expect_bool_eq(array_first3(reversed), expected_first) + expect_bool_eq(array_last3(reversed), expected_last)
}

pub fn expect_bool_array_reverse4(values: [Bool; 4], expected_first: Bool, expected_last: Bool) -> Int {
    let reversed: [Bool; 4] = array_reverse4(values)
    return expect_bool_eq(array_first4(reversed), expected_first) + expect_bool_eq(array_last4(reversed), expected_last)
}

pub fn expect_bool_array_reverse5(values: [Bool; 5], expected_first: Bool, expected_last: Bool) -> Int {
    let reversed: [Bool; 5] = array_reverse5(values)
    return expect_bool_eq(array_first5(reversed), expected_first) + expect_bool_eq(array_last5(reversed), expected_last)
}

pub fn expect_int_array_repeat3(value: Int, expected: Int) -> Int {
    let repeated: [Int; 3] = array_repeat3(value)
    return expect_int_eq(array_first3(repeated), expected) + expect_int_eq(array_last3(repeated), expected)
}

pub fn expect_int_array_repeat4(value: Int, expected: Int) -> Int {
    let repeated: [Int; 4] = array_repeat4(value)
    return expect_int_eq(array_first4(repeated), expected) + expect_int_eq(array_last4(repeated), expected)
}

pub fn expect_int_array_repeat5(value: Int, expected: Int) -> Int {
    let repeated: [Int; 5] = array_repeat5(value)
    return expect_int_eq(array_first5(repeated), expected) + expect_int_eq(array_last5(repeated), expected)
}

pub fn expect_bool_array_repeat3(value: Bool, expected: Bool) -> Int {
    let repeated: [Bool; 3] = array_repeat3(value)
    return expect_bool_eq(array_first3(repeated), expected) + expect_bool_eq(array_last3(repeated), expected)
}

pub fn expect_bool_array_repeat4(value: Bool, expected: Bool) -> Int {
    let repeated: [Bool; 4] = array_repeat4(value)
    return expect_bool_eq(array_first4(repeated), expected) + expect_bool_eq(array_last4(repeated), expected)
}

pub fn expect_bool_array_repeat5(value: Bool, expected: Bool) -> Int {
    let repeated: [Bool; 5] = array_repeat5(value)
    return expect_bool_eq(array_first5(repeated), expected) + expect_bool_eq(array_last5(repeated), expected)
}

pub fn expect_int_array_sum3(values: [Int; 3], expected: Int) -> Int {
    return expect_int_eq(array_sum3_int(values), expected)
}

pub fn expect_int_array_sum4(values: [Int; 4], expected: Int) -> Int {
    return expect_int_eq(array_sum4_int(values), expected)
}

pub fn expect_int_array_sum5(values: [Int; 5], expected: Int) -> Int {
    return expect_int_eq(array_sum5_int(values), expected)
}

pub fn expect_int_array_product3(values: [Int; 3], expected: Int) -> Int {
    return expect_int_eq(array_product3_int(values), expected)
}

pub fn expect_int_array_product4(values: [Int; 4], expected: Int) -> Int {
    return expect_int_eq(array_product4_int(values), expected)
}

pub fn expect_int_array_product5(values: [Int; 5], expected: Int) -> Int {
    return expect_int_eq(array_product5_int(values), expected)
}

pub fn expect_int_array_max3(values: [Int; 3], expected: Int) -> Int {
    return expect_int_eq(array_max3_int(values), expected)
}

pub fn expect_int_array_max4(values: [Int; 4], expected: Int) -> Int {
    return expect_int_eq(array_max4_int(values), expected)
}

pub fn expect_int_array_max5(values: [Int; 5], expected: Int) -> Int {
    return expect_int_eq(array_max5_int(values), expected)
}

pub fn expect_int_array_min3(values: [Int; 3], expected: Int) -> Int {
    return expect_int_eq(array_min3_int(values), expected)
}

pub fn expect_int_array_min4(values: [Int; 4], expected: Int) -> Int {
    return expect_int_eq(array_min4_int(values), expected)
}

pub fn expect_int_array_min5(values: [Int; 5], expected: Int) -> Int {
    return expect_int_eq(array_min5_int(values), expected)
}

pub fn expect_bool_array_all3(values: [Bool; 3], expected: Bool) -> Int {
    return expect_bool_eq(array_all3_bool(values), expected)
}

pub fn expect_bool_array_all4(values: [Bool; 4], expected: Bool) -> Int {
    return expect_bool_eq(array_all4_bool(values), expected)
}

pub fn expect_bool_array_all5(values: [Bool; 5], expected: Bool) -> Int {
    return expect_bool_eq(array_all5_bool(values), expected)
}

pub fn expect_bool_array_any3(values: [Bool; 3], expected: Bool) -> Int {
    return expect_bool_eq(array_any3_bool(values), expected)
}

pub fn expect_bool_array_any4(values: [Bool; 4], expected: Bool) -> Int {
    return expect_bool_eq(array_any4_bool(values), expected)
}

pub fn expect_bool_array_any5(values: [Bool; 5], expected: Bool) -> Int {
    return expect_bool_eq(array_any5_bool(values), expected)
}

pub fn expect_bool_array_none3(values: [Bool; 3], expected: Bool) -> Int {
    return expect_bool_eq(array_none3_bool(values), expected)
}

pub fn expect_bool_array_none4(values: [Bool; 4], expected: Bool) -> Int {
    return expect_bool_eq(array_none4_bool(values), expected)
}

pub fn expect_bool_array_none5(values: [Bool; 5], expected: Bool) -> Int {
    return expect_bool_eq(array_none5_bool(values), expected)
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

pub fn expect_int_max5(first: Int, second: Int, third: Int, fourth: Int, fifth: Int, expected: Int) -> Int {
    if max5_int(first, second, third, fourth, fifth) == expected {
        return 0
    }
    return 1
}

pub fn expect_int_min5(first: Int, second: Int, third: Int, fourth: Int, fifth: Int, expected: Int) -> Int {
    if min5_int(first, second, third, fourth, fifth) == expected {
        return 0
    }
    return 1
}

pub fn expect_int_median3(first: Int, second: Int, third: Int, expected: Int) -> Int {
    if median3_int(first, second, third) == expected {
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

pub fn expect_int_sum5(first: Int, second: Int, third: Int, fourth: Int, fifth: Int, expected: Int) -> Int {
    if sum5_int(first, second, third, fourth, fifth) == expected {
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

pub fn expect_int_product5(first: Int, second: Int, third: Int, fourth: Int, fifth: Int, expected: Int) -> Int {
    if product5_int(first, second, third, fourth, fifth) == expected {
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

pub fn expect_int_average4(first: Int, second: Int, third: Int, fourth: Int, expected: Int) -> Int {
    if average4_int(first, second, third, fourth) == expected {
        return 0
    }
    return 1
}

pub fn expect_int_average5(first: Int, second: Int, third: Int, fourth: Int, fifth: Int, expected: Int) -> Int {
    if average5_int(first, second, third, fourth, fifth) == expected {
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

pub fn expect_int_ascending(first: Int, second: Int, third: Int) -> Int {
    if is_ascending_int(first, second, third) {
        return 0
    }
    return 1
}

pub fn expect_int_ascending4(first: Int, second: Int, third: Int, fourth: Int) -> Int {
    if is_ascending4_int(first, second, third, fourth) {
        return 0
    }
    return 1
}

pub fn expect_int_ascending5(first: Int, second: Int, third: Int, fourth: Int, fifth: Int) -> Int {
    if is_ascending5_int(first, second, third, fourth, fifth) {
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

pub fn expect_int_strictly_ascending4(first: Int, second: Int, third: Int, fourth: Int) -> Int {
    if is_strictly_ascending4_int(first, second, third, fourth) {
        return 0
    }
    return 1
}

pub fn expect_int_strictly_ascending5(first: Int, second: Int, third: Int, fourth: Int, fifth: Int) -> Int {
    if is_strictly_ascending5_int(first, second, third, fourth, fifth) {
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

pub fn expect_int_descending4(first: Int, second: Int, third: Int, fourth: Int) -> Int {
    if is_descending4_int(first, second, third, fourth) {
        return 0
    }
    return 1
}

pub fn expect_int_descending5(first: Int, second: Int, third: Int, fourth: Int, fifth: Int) -> Int {
    if is_descending5_int(first, second, third, fourth, fifth) {
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

pub fn expect_int_strictly_descending4(first: Int, second: Int, third: Int, fourth: Int) -> Int {
    if is_strictly_descending4_int(first, second, third, fourth) {
        return 0
    }
    return 1
}

pub fn expect_int_strictly_descending5(first: Int, second: Int, third: Int, fourth: Int, fifth: Int) -> Int {
    if is_strictly_descending5_int(first, second, third, fourth, fifth) {
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

pub fn expect_int_option_some(value: IntOption, expected: Int) -> Int {
    if option_is_some_int(value) && option_unwrap_or_int(value, expected) == expected {
        return 0
    }
    return 1
}

pub fn expect_int_option_none(value: IntOption) -> Int {
    if option_is_none_int(value) {
        return 0
    }
    return 1
}

pub fn expect_bool_option_some(value: BoolOption, expected: Bool) -> Int {
    if option_is_some_bool(value) && option_unwrap_or_bool(value, expected) == expected {
        return 0
    }
    return 1
}

pub fn expect_bool_option_none(value: BoolOption) -> Int {
    if option_is_none_bool(value) {
        return 0
    }
    return 1
}

pub fn expect_int_option_or(value: IntOption, fallback: IntOption, expected: Int) -> Int {
    if option_unwrap_or_int(option_or_int(value, fallback), expected) == expected {
        return 0
    }
    return 1
}

pub fn expect_bool_option_or(value: BoolOption, fallback: BoolOption, expected: Bool) -> Int {
    if option_unwrap_or_bool(option_or_bool(value, fallback), expected) == expected {
        return 0
    }
    return 1
}

pub fn expect_generic_int_option_some(value: Option[Int], expected: Int) -> Int {
    return match value {
        Option.Some(inner) => expect_int_eq(inner, expected),
        Option.None => 1,
    }
}

pub fn expect_generic_int_option_none(value: Option[Int]) -> Int {
    return match value {
        Option.Some(_) => 1,
        Option.None => 0,
    }
}

pub fn expect_generic_int_option_or(value: Option[Int], fallback: Option[Int], expected: Int) -> Int {
    return match value {
        Option.Some(inner) => expect_int_eq(inner, expected),
        Option.None => expect_generic_int_option_some(fallback, expected),
    }
}

pub fn expect_generic_bool_option_some(value: Option[Bool], expected: Bool) -> Int {
    return match value {
        Option.Some(inner) => expect_bool_eq(inner, expected),
        Option.None => 1,
    }
}

pub fn expect_generic_bool_option_none(value: Option[Bool]) -> Int {
    return match value {
        Option.Some(_) => 1,
        Option.None => 0,
    }
}

pub fn expect_generic_bool_option_or(value: Option[Bool], fallback: Option[Bool], expected: Bool) -> Int {
    return match value {
        Option.Some(inner) => expect_bool_eq(inner, expected),
        Option.None => expect_generic_bool_option_some(fallback, expected),
    }
}

pub fn expect_int_result_ok(value: IntResult, expected: Int) -> Int {
    if result_is_ok_int(value) && result_unwrap_or_int(value, expected) == expected {
        return 0
    }
    return 1
}

pub fn expect_int_result_err(value: IntResult, expected_error: Int) -> Int {
    if result_is_err_int(value) && result_error_or_zero_int(value) == expected_error {
        return 0
    }
    return 1
}

pub fn expect_bool_result_ok(value: BoolResult, expected: Bool) -> Int {
    if result_is_ok_bool(value) && result_unwrap_or_bool(value, expected) == expected {
        return 0
    }
    return 1
}

pub fn expect_bool_result_err(value: BoolResult, expected_error: Int) -> Int {
    if result_is_err_bool(value) && result_error_or_zero_bool(value) == expected_error {
        return 0
    }
    return 1
}

pub fn expect_int_result_or(value: IntResult, fallback: IntResult, expected: Int) -> Int {
    if result_unwrap_or_int(result_or_int(value, fallback), expected) == expected {
        return 0
    }
    return 1
}

pub fn expect_bool_result_or(value: BoolResult, fallback: BoolResult, expected: Bool) -> Int {
    if result_unwrap_or_bool(result_or_bool(value, fallback), expected) == expected {
        return 0
    }
    return 1
}

pub fn expect_generic_int_result_ok(value: Result[Int, Int], expected: Int) -> Int {
    return match value {
        Result.Ok(inner) => expect_int_eq(inner, expected),
        Result.Err(_) => 1,
    }
}

pub fn expect_generic_int_result_err(value: Result[Int, Int], expected_error: Int) -> Int {
    return match value {
        Result.Ok(_) => 1,
        Result.Err(error) => expect_int_eq(error, expected_error),
    }
}

pub fn expect_generic_int_result_or(value: Result[Int, Int], fallback: Result[Int, Int], expected: Int) -> Int {
    return match value {
        Result.Ok(inner) => expect_int_eq(inner, expected),
        Result.Err(_) => expect_generic_int_result_ok(fallback, expected),
    }
}

pub fn expect_generic_int_result_error(value: Result[Int, Int], fallback_error: Int, expected_error: Int) -> Int {
    return match value {
        Result.Ok(_) => expect_int_eq(fallback_error, expected_error),
        Result.Err(error) => expect_int_eq(error, expected_error),
    }
}

pub fn expect_generic_int_result_to_option_some(value: Result[Int, Int], expected: Int) -> Int {
    return expect_generic_int_option_some(result_to_option(value), expected)
}

pub fn expect_generic_int_result_to_option_none(value: Result[Int, Int]) -> Int {
    return expect_generic_int_option_none(result_to_option(value))
}

pub fn expect_generic_int_result_error_some(value: Result[Int, Int], expected_error: Int) -> Int {
    return expect_generic_int_option_some(result_error_to_option(value), expected_error)
}

pub fn expect_generic_int_result_error_none(value: Result[Int, Int]) -> Int {
    return expect_generic_int_option_none(result_error_to_option(value))
}

pub fn expect_generic_int_option_ok_or(value: Option[Int], error: Int, expected: Int) -> Int {
    return expect_generic_int_result_ok(result_ok_or(value, error), expected)
}

pub fn expect_generic_int_option_ok_or_err(value: Option[Int], error: Int) -> Int {
    return expect_generic_int_result_err(result_ok_or(value, error), error)
}

pub fn expect_generic_bool_result_ok(value: Result[Bool, Int], expected: Bool) -> Int {
    return match value {
        Result.Ok(inner) => expect_bool_eq(inner, expected),
        Result.Err(_) => 1,
    }
}

pub fn expect_generic_bool_result_err(value: Result[Bool, Int], expected_error: Int) -> Int {
    return match value {
        Result.Ok(_) => 1,
        Result.Err(error) => expect_int_eq(error, expected_error),
    }
}

pub fn expect_generic_bool_result_or(value: Result[Bool, Int], fallback: Result[Bool, Int], expected: Bool) -> Int {
    return match value {
        Result.Ok(inner) => expect_bool_eq(inner, expected),
        Result.Err(_) => expect_generic_bool_result_ok(fallback, expected),
    }
}

pub fn expect_generic_bool_result_error(value: Result[Bool, Int], fallback_error: Int, expected_error: Int) -> Int {
    return match value {
        Result.Ok(_) => expect_int_eq(fallback_error, expected_error),
        Result.Err(error) => expect_int_eq(error, expected_error),
    }
}

pub fn expect_generic_bool_result_to_option_some(value: Result[Bool, Int], expected: Bool) -> Int {
    return expect_generic_bool_option_some(result_to_option(value), expected)
}

pub fn expect_generic_bool_result_to_option_none(value: Result[Bool, Int]) -> Int {
    return expect_generic_bool_option_none(result_to_option(value))
}

pub fn expect_generic_bool_result_error_some(value: Result[Bool, Int], expected_error: Int) -> Int {
    return expect_generic_int_option_some(result_error_to_option(value), expected_error)
}

pub fn expect_generic_bool_result_error_none(value: Result[Bool, Int]) -> Int {
    return expect_generic_int_option_none(result_error_to_option(value))
}

pub fn expect_generic_bool_option_ok_or(value: Option[Bool], error: Int, expected: Bool) -> Int {
    return expect_generic_bool_result_ok(result_ok_or(value, error), expected)
}

pub fn expect_generic_bool_option_ok_or_err(value: Option[Bool], error: Int) -> Int {
    return expect_generic_bool_result_err(result_ok_or(value, error), error)
}

pub fn expect_int_result_to_option_some(value: IntResult, expected: Int) -> Int {
    return expect_int_option_some(result_to_option_int(value), expected)
}

pub fn expect_int_result_to_option_none(value: IntResult) -> Int {
    return expect_int_option_none(result_to_option_int(value))
}

pub fn expect_bool_result_to_option_some(value: BoolResult, expected: Bool) -> Int {
    return expect_bool_option_some(result_to_option_bool(value), expected)
}

pub fn expect_bool_result_to_option_none(value: BoolResult) -> Int {
    return expect_bool_option_none(result_to_option_bool(value))
}

pub fn expect_int_result_error_some(value: IntResult, expected_error: Int) -> Int {
    return expect_int_option_some(result_error_to_option_int(value), expected_error)
}

pub fn expect_int_result_error_none(value: IntResult) -> Int {
    return expect_int_option_none(result_error_to_option_int(value))
}

pub fn expect_bool_result_error_some(value: BoolResult, expected_error: Int) -> Int {
    return expect_int_option_some(result_error_to_option_bool(value), expected_error)
}

pub fn expect_bool_result_error_none(value: BoolResult) -> Int {
    return expect_int_option_none(result_error_to_option_bool(value))
}

pub fn expect_int_option_ok_or(value: IntOption, error: Int, expected: Int) -> Int {
    return expect_int_result_ok(result_ok_or_int(value, error), expected)
}

pub fn expect_int_option_ok_or_err(value: IntOption, error: Int) -> Int {
    return expect_int_result_err(result_ok_or_int(value, error), error)
}

pub fn expect_bool_option_ok_or(value: BoolOption, error: Int, expected: Bool) -> Int {
    return expect_bool_result_ok(result_ok_or_bool(value, error), expected)
}

pub fn expect_bool_option_ok_or_err(value: BoolOption, error: Int) -> Int {
    return expect_bool_result_err(result_ok_or_bool(value, error), error)
}
