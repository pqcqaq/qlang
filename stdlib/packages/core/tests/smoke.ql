use std.core.abs_diff_int as abs_diff_int
use std.core.abs_int as abs_int
use std.core.all_bools as all_bools
use std.core.any_bools as any_bools
use std.core.average_ints as average_ints
use std.core.and_bool as and_bool
use std.core.bool_to_int as bool_to_int
use std.core.clamp_int as clamp_int
use std.core.clamp_bounds_int as clamp_bounds_int
use std.core.clamp_max_int as clamp_max_int
use std.core.clamp_min_int as clamp_min_int
use std.core.compare_int as compare_int
use std.core.distance_to_bounds_int as distance_to_bounds_int
use std.core.distance_to_range_int as distance_to_range_int
use std.core.has_remainder_int as has_remainder_int
use std.core.in_bounds_int as in_bounds_int
use std.core.in_exclusive_bounds_int as in_exclusive_bounds_int
use std.core.in_exclusive_range_int as in_exclusive_range_int
use std.core.in_range_int as in_range_int
use std.core.is_descending_ints as is_descending_ints
use std.core.is_divisible_by_int as is_divisible_by_int
use std.core.is_even_int as is_even_int
use std.core.is_factor_of_int as is_factor_of_int
use std.core.is_negative_int as is_negative_int
use std.core.is_nonnegative_int as is_nonnegative_int
use std.core.is_nonpositive_int as is_nonpositive_int
use std.core.is_nonzero_int as is_nonzero_int
use std.core.is_not_within_int as is_not_within_int
use std.core.is_odd_int as is_odd_int
use std.core.is_outside_bounds_int as is_outside_bounds_int
use std.core.is_outside_range_int as is_outside_range_int
use std.core.is_positive_int as is_positive_int
use std.core.is_strictly_descending_ints as is_strictly_descending_ints
use std.core.is_ascending_ints as is_ascending_ints
use std.core.is_strictly_ascending_ints as is_strictly_ascending_ints
use std.core.is_within_int as is_within_int
use std.core.is_zero_int as is_zero_int
use std.core.lower_bound_int as lower_bound_int
use std.core.max_int as max_int
use std.core.max_ints as max_ints
use std.core.median_ints as median_ints
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
use std.core.implies_bool as implies_bool

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
    let extrema_status = sum_statuses([check_int(max_ints([3, 9, 5]), 9), check_int(max_ints([3, 9, 5, 7]), 9), check_int(max_ints([3, 9, 5, 7, 11]), 11), check_int(min_ints([3, 9, 5]), 3), check_int(min_ints([3, 9, 5, 7]), 3), check_int(min_ints([3, 9, 5, 7, 1]), 1)])
    let compare_status = sum_statuses([check_int(median_ints([9, 3, 5]), 5), check_int(median_ints([9, 3, 5, 7]), 7), check_int(compare_int(9, 3), 1), 0, 0, 0])
    let range_status = sum_statuses([check_bool(in_bounds_int(5, 9, 3), true), check_bool(is_outside_range_int(2, 3, 9), true), check_bool(is_outside_range_int(5, 3, 9), false), check_bool(is_outside_bounds_int(10, 9, 3), true), check_bool(is_outside_bounds_int(5, 9, 3), false), check_bool(is_within_int(11, 10, 1), true)])
    let order_status = sum_statuses([check_bool(is_not_within_int(12, 10, 1), true), check_bool(is_not_within_int(10, 10, 0), false), check_bool(is_descending_ints([9, 9, 3]), true), check_bool(is_descending_ints([3, 9, 5]), false), check_bool(is_strictly_descending_ints([9, 5, 3]), true), check_bool(is_strictly_descending_ints([9, 9, 3]), false)])
    let ascending_multi_status = sum_statuses([check_bool(is_ascending_ints([3, 5, 5, 9]), true), check_bool(is_ascending_ints([3, 5, 5, 9, 10]), true), check_bool(is_strictly_ascending_ints([3, 5, 7, 9]), true), check_bool(is_strictly_ascending_ints([3, 5, 7, 9, 11]), true), check_bool(is_strictly_ascending_ints([3, 5, 5, 9]), false), check_bool(is_strictly_ascending_ints([3, 5, 7, 9, 9]), false)])
    let descending_multi_status = sum_statuses([check_bool(is_descending_ints([9, 7, 7, 3]), true), check_bool(is_descending_ints([11, 9, 7, 7, 3]), true), check_bool(is_strictly_descending_ints([9, 7, 5, 3]), true), check_bool(is_strictly_descending_ints([11, 9, 7, 5, 3]), true), check_bool(is_strictly_descending_ints([9, 7, 7, 3]), false), check_bool(is_strictly_descending_ints([11, 9, 7, 7, 3]), false)])
    let generic_order_status = sum_statuses([check_bool(is_ascending_ints([3, 5, 5, 9, 10, 10]), true), check_bool(is_ascending_ints([3, 9, 5, 7, 8, 9]), false), check_bool(is_strictly_ascending_ints([3, 5, 7, 9, 11, 13]), true), check_bool(is_strictly_ascending_ints([3, 5, 5, 9, 11, 13]), false), check_bool(is_descending_ints([13, 11, 11, 7, 5, 3]), true), check_bool(is_descending_ints([13, 7, 11, 5, 3, 2]), false)])
    let generic_strict_order_status = sum_statuses([check_bool(is_strictly_descending_ints([13, 11, 9, 7, 5, 3]), true), check_bool(is_strictly_descending_ints([13, 11, 11, 7, 5, 3]), false), check_bool(is_ascending_ints([2, 3, 4, 5, 6, 7]), true), check_bool(is_descending_ints([7, 6, 5, 4, 3, 2]), true), 0, 0])
    let transform_status = sum_statuses([check_int(clamp_bounds_int(12, 9, 3), 9), check_int(abs_int(0 - 7), 7), check_int(abs_diff_int(3, 9), 6), check_int(range_span_int(9, 3), 6), check_int(lower_bound_int(9, 3), 3), check_int(upper_bound_int(9, 3), 9)])
    let scalar_clamp_status = sum_statuses([check_int(max_int(3, 9), 9), check_int(min_int(3, 9), 3), check_int(clamp_int(12, 3, 9), 9), check_int(clamp_int(5, 3, 9), 5), check_int(clamp_min_int(2, 3), 3), check_int(clamp_max_int(12, 9), 9)])
    let boundary_status = sum_statuses([check_int(distance_to_range_int(2, 3, 9), 1), check_int(distance_to_bounds_int(10, 9, 3), 1), check_int(distance_to_range_int(5, 3, 9), 0), check_int(sign_int(0 - 5), 0 - 1), check_int(sign_int(0), 0), check_int(sign_int(5), 1)])
    let int_predicate_status = sum_statuses([check_bool(is_zero_int(0), true), check_bool(is_nonzero_int(5), true), check_bool(is_positive_int(5), true), check_bool(is_nonnegative_int(0), true), check_bool(is_negative_int(0 - 5), true), check_bool(is_nonpositive_int(0), true)])
    let parity_status = sum_statuses([check_bool(is_even_int(8), true), check_bool(is_even_int(7), false), check_bool(is_odd_int(7), true), check_bool(is_odd_int(8), false), check_bool(is_divisible_by_int(21, 7), true), check_bool(is_divisible_by_int(21, 0), false)])
    let range_predicate_status = sum_statuses([check_bool(in_range_int(5, 3, 9), true), check_bool(in_range_int(10, 3, 9), false), check_bool(in_exclusive_range_int(5, 3, 9), true), check_bool(in_exclusive_range_int(3, 3, 9), false), check_bool(in_exclusive_bounds_int(5, 9, 3), true), check_bool(in_exclusive_bounds_int(9, 9, 3), false)])
    let aggregate_status = sum_statuses([check_int(sum_ints([2, 3, 4]), 9), check_int(sum_ints([2, 3, 4, 5]), 14), check_int(sum_ints([2, 3, 4, 5, 6]), 20), check_int(product_ints([2, 3, 4]), 24), check_int(product_ints([2, 3, 4, 5]), 120), check_int(product_ints([2, 3, 4, 5, 6]), 720)])
    let average_status = sum_statuses([check_int(average_ints([5, 8]), 6), check_int(average_ints([3, 6, 9]), 6), check_int(average_ints([2, 4, 6, 8]), 5), check_int(average_ints([2, 4, 6, 8, 10]), 6), 0, 0])
    let generic_aggregate_status = sum_statuses([check_int(sum_ints([2, 3, 4, 5]), 14), check_int(product_ints([2, 3, 4]), 24), check_int(max_ints([3, 9, 5, 7]), 9), check_int(min_ints([3, 9, 5, 7]), 3), check_int(average_ints([2, 4, 6, 8]), 5), 0])
    let division_status = sum_statuses([check_int(quotient_or_zero_int(21, 7), 3), check_int(quotient_or_zero_int(21, 0), 0), check_int(remainder_or_zero_int(22, 7), 1), check_int(remainder_or_zero_int(22, 0), 0), check_bool(has_remainder_int(22, 7), true), check_bool(is_factor_of_int(7, 21), true)])
    let bool_aggregate_status = sum_statuses([check_bool(all_bools([true, true, true]), true), check_bool(all_bools([true, true, true, false]), false), check_bool(all_bools([true, true, true, true, true]), true), check_bool(any_bools([false, false, true]), true), check_bool(any_bools([false, false, false, false]), false), check_bool(any_bools([false, false, false, false, true]), true)])
    let bool_none_status = sum_statuses([check_bool(none_bools([false, false, false]), true), check_bool(none_bools([false, false, true, false]), false), check_bool(none_bools([false, false, false, false, false]), true), 0, 0, 0])
    let bool_operator_status = sum_statuses([check_bool(not_bool(false), true), check_bool(and_bool(true, false), false), check_bool(or_bool(false, true), true), check_bool(xor_bool(true, false), true), check_bool(xor_bool(true, true), false), check_bool(implies_bool(true, false), false)])
    let generic_bool_status = sum_statuses([check_bool(all_bools([true, true, true]), true), check_bool(all_bools([true, false, true, true]), false), check_bool(any_bools([false, false, true]), true), check_bool(any_bools([false, false, false, false]), false), check_bool(none_bools([false, false, false]), true), check_bool(none_bools([false, true, false]), false)])
    return extrema_status + compare_status + range_status + order_status + ascending_multi_status + descending_multi_status + generic_order_status + generic_strict_order_status + transform_status + scalar_clamp_status + boundary_status + int_predicate_status + parity_status + range_predicate_status + aggregate_status + average_status + generic_aggregate_status + division_status + bool_aggregate_status + bool_none_status + bool_operator_status + generic_bool_status + check_int(bool_to_int(true), 1)
}
