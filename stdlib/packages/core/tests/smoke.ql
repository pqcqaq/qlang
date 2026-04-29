use std.core.abs_diff_int as abs_diff_int
use std.core.bool_to_int as bool_to_int
use std.core.clamp_bounds_int as clamp_bounds_int
use std.core.compare_int as compare_int
use std.core.in_bounds_int as in_bounds_int
use std.core.is_descending_int as is_descending_int
use std.core.is_not_within_int as is_not_within_int
use std.core.is_outside_bounds_int as is_outside_bounds_int
use std.core.is_outside_range_int as is_outside_range_int
use std.core.is_strictly_descending_int as is_strictly_descending_int
use std.core.is_within_int as is_within_int
use std.core.max3_int as max3_int
use std.core.median3_int as median3_int
use std.core.min3_int as min3_int

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

    return extrema_status + range_status + order_status + check_int(bool_to_int(true), 1)
}
