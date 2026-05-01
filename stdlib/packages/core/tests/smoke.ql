use std.core.abs_diff_int as abs_diff_int
use std.core.abs_int as abs_int
use std.core.all3_bool as all3_bool
use std.core.all4_bool as all4_bool
use std.core.all5_bool as all5_bool
use std.core.any3_bool as any3_bool
use std.core.any4_bool as any4_bool
use std.core.any5_bool as any5_bool
use std.core.average2_int as average2_int
use std.core.average3_int as average3_int
use std.core.average4_int as average4_int
use std.core.average5_int as average5_int
use std.core.bool_to_int as bool_to_int
use std.core.clamp_bounds_int as clamp_bounds_int
use std.core.compare_int as compare_int
use std.core.distance_to_bounds_int as distance_to_bounds_int
use std.core.distance_to_range_int as distance_to_range_int
use std.core.has_remainder_int as has_remainder_int
use std.core.in_bounds_int as in_bounds_int
use std.core.is_descending_int as is_descending_int
use std.core.is_factor_of_int as is_factor_of_int
use std.core.is_not_within_int as is_not_within_int
use std.core.is_outside_bounds_int as is_outside_bounds_int
use std.core.is_outside_range_int as is_outside_range_int
use std.core.is_strictly_descending_int as is_strictly_descending_int
use std.core.is_within_int as is_within_int
use std.core.lower_bound_int as lower_bound_int
use std.core.max3_int as max3_int
use std.core.max4_int as max4_int
use std.core.max5_int as max5_int
use std.core.median3_int as median3_int
use std.core.min3_int as min3_int
use std.core.min4_int as min4_int
use std.core.min5_int as min5_int
use std.core.none3_bool as none3_bool
use std.core.none4_bool as none4_bool
use std.core.none5_bool as none5_bool
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
    let extrema_status = sum6(check_int(max3_int(3, 9, 5), 9), check_int(max4_int(3, 9, 5, 7), 9), check_int(max5_int(3, 9, 5, 7, 11), 11), check_int(min3_int(3, 9, 5), 3), check_int(min4_int(3, 9, 5, 7), 3), check_int(min5_int(3, 9, 5, 7, 1), 1))
    let compare_status = sum6(check_int(median3_int(9, 3, 5), 5), check_int(compare_int(9, 3), 1), 0, 0, 0, 0)
    let range_status = sum6(check_bool(in_bounds_int(5, 9, 3), true), check_bool(is_outside_range_int(2, 3, 9), true), check_bool(is_outside_range_int(5, 3, 9), false), check_bool(is_outside_bounds_int(10, 9, 3), true), check_bool(is_outside_bounds_int(5, 9, 3), false), check_bool(is_within_int(11, 10, 1), true))
    let order_status = sum6(check_bool(is_not_within_int(12, 10, 1), true), check_bool(is_not_within_int(10, 10, 0), false), check_bool(is_descending_int(9, 9, 3), true), check_bool(is_descending_int(3, 9, 5), false), check_bool(is_strictly_descending_int(9, 5, 3), true), check_bool(is_strictly_descending_int(9, 9, 3), false))
    let transform_status = sum6(check_int(clamp_bounds_int(12, 9, 3), 9), check_int(abs_int(0 - 7), 7), check_int(abs_diff_int(3, 9), 6), check_int(range_span_int(9, 3), 6), check_int(lower_bound_int(9, 3), 3), check_int(upper_bound_int(9, 3), 9))
    let boundary_status = sum6(check_int(distance_to_range_int(2, 3, 9), 1), check_int(distance_to_bounds_int(10, 9, 3), 1), check_int(distance_to_range_int(5, 3, 9), 0), check_int(sign_int(0 - 5), 0 - 1), check_int(sign_int(0), 0), check_int(sign_int(5), 1))
    let aggregate_status = sum6(check_int(sum3_int(2, 3, 4), 9), check_int(sum4_int(2, 3, 4, 5), 14), check_int(sum5_int(2, 3, 4, 5, 6), 20), check_int(product3_int(2, 3, 4), 24), check_int(product4_int(2, 3, 4, 5), 120), check_int(product5_int(2, 3, 4, 5, 6), 720))
    let average_status = sum6(check_int(average2_int(5, 8), 6), check_int(average3_int(3, 6, 9), 6), check_int(average4_int(2, 4, 6, 8), 5), check_int(average5_int(2, 4, 6, 8, 10), 6), 0, 0)
    let division_status = sum6(check_int(quotient_or_zero_int(21, 7), 3), check_int(quotient_or_zero_int(21, 0), 0), check_int(remainder_or_zero_int(22, 7), 1), check_int(remainder_or_zero_int(22, 0), 0), check_bool(has_remainder_int(22, 7), true), check_bool(is_factor_of_int(7, 21), true))
    let bool_aggregate_status = sum6(check_bool(all3_bool(true, true, true), true), check_bool(all4_bool(true, true, true, false), false), check_bool(all5_bool(true, true, true, true, true), true), check_bool(any3_bool(false, false, true), true), check_bool(any4_bool(false, false, false, false), false), check_bool(any5_bool(false, false, false, false, true), true))
    let bool_none_status = sum6(check_bool(none3_bool(false, false, false), true), check_bool(none4_bool(false, false, true, false), false), check_bool(none5_bool(false, false, false, false, false), true), 0, 0, 0)

    return extrema_status + compare_status + range_status + order_status + transform_status + boundary_status + aggregate_status + average_status + division_status + bool_aggregate_status + bool_none_status + check_int(bool_to_int(true), 1)
}
