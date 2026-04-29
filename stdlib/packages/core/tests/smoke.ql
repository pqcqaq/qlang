use std.core.abs_diff_int as abs_diff_int
use std.core.average2_int as average2_int
use std.core.average3_int as average3_int
use std.core.bool_to_int as bool_to_int
use std.core.clamp_bounds_int as clamp_bounds_int
use std.core.compare_int as compare_int
use std.core.distance_to_bounds_int as distance_to_bounds_int
use std.core.distance_to_range_int as distance_to_range_int
use std.core.in_bounds_int as in_bounds_int
use std.core.is_descending_int as is_descending_int
use std.core.is_not_within_int as is_not_within_int
use std.core.is_outside_bounds_int as is_outside_bounds_int
use std.core.is_outside_range_int as is_outside_range_int
use std.core.is_strictly_descending_int as is_strictly_descending_int
use std.core.is_within_int as is_within_int
use std.core.lower_bound_int as lower_bound_int
use std.core.max3_int as max3_int
use std.core.median3_int as median3_int
use std.core.min3_int as min3_int
use std.core.product3_int as product3_int
use std.core.product4_int as product4_int
use std.core.sum3_int as sum3_int
use std.core.sum4_int as sum4_int
use std.core.upper_bound_int as upper_bound_int

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
    let extrema_status = sum6(check_int(max3_int(3, 9, 5), 9), check_int(min3_int(3, 9, 5), 3), check_int(median3_int(9, 3, 5), 5), check_int(clamp_bounds_int(12, 9, 3), 9), check_int(abs_diff_int(3, 9), 6), check_int(compare_int(9, 3), 1))
    let range_status = sum6(check_bool(in_bounds_int(5, 9, 3), true), check_bool(is_outside_range_int(2, 3, 9), true), check_bool(is_outside_range_int(5, 3, 9), false), check_bool(is_outside_bounds_int(10, 9, 3), true), check_bool(is_outside_bounds_int(5, 9, 3), false), check_bool(is_within_int(11, 10, 1), true))
    let order_status = sum6(check_bool(is_not_within_int(12, 10, 1), true), check_bool(is_not_within_int(10, 10, 0), false), check_bool(is_descending_int(9, 9, 3), true), check_bool(is_descending_int(3, 9, 5), false), check_bool(is_strictly_descending_int(9, 5, 3), true), check_bool(is_strictly_descending_int(9, 9, 3), false))
    let boundary_status = sum6(check_int(lower_bound_int(9, 3), 3), check_int(upper_bound_int(9, 3), 9), check_int(distance_to_range_int(2, 3, 9), 1), check_int(distance_to_range_int(5, 3, 9), 0), check_int(distance_to_bounds_int(10, 9, 3), 1), check_int(distance_to_bounds_int(5, 9, 3), 0))
    let aggregate_status = sum6(check_int(sum3_int(2, 3, 4), 9), check_int(sum4_int(2, 3, 4, 5), 14), check_int(product3_int(2, 3, 4), 24), check_int(product4_int(2, 3, 4, 5), 120), check_int(average2_int(5, 8), 6), check_int(average3_int(3, 6, 9), 6))

    return extrema_status + range_status + order_status + boundary_status + aggregate_status + check_int(bool_to_int(true), 1)
}
