use std.array.all3_bool_array as all3_bool_array
use std.array.all4_bool_array as all4_bool_array
use std.array.all5_bool_array as all5_bool_array
use std.array.any3_bool_array as any3_bool_array
use std.array.any4_bool_array as any4_bool_array
use std.array.any5_bool_array as any5_bool_array
use std.array.first3_array as first3_array
use std.array.first4_array as first4_array
use std.array.first5_array as first5_array
use std.array.last3_array as last3_array
use std.array.last4_array as last4_array
use std.array.last5_array as last5_array
use std.array.max3_int_array as max3_int_array
use std.array.max4_int_array as max4_int_array
use std.array.max5_int_array as max5_int_array
use std.array.min3_int_array as min3_int_array
use std.array.min4_int_array as min4_int_array
use std.array.min5_int_array as min5_int_array
use std.array.none3_bool_array as none3_bool_array
use std.array.none4_bool_array as none4_bool_array
use std.array.none5_bool_array as none5_bool_array
use std.array.product3_int_array as product3_int_array
use std.array.product4_int_array as product4_int_array
use std.array.product5_int_array as product5_int_array
use std.array.sum3_int_array as sum3_int_array
use std.array.sum4_int_array as sum4_int_array
use std.array.sum5_int_array as sum5_int_array

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
    let generic_int_status = sum6(check_int(first3_array([2, 3, 4]), 2), check_int(first4_array([2, 3, 4, 5]), 2), check_int(first5_array([2, 3, 4, 5, 6]), 2), check_int(last3_array([2, 3, 4]), 4), check_int(last4_array([2, 3, 4, 5]), 5), check_int(last5_array([2, 3, 4, 5, 6]), 6))
    let generic_bool_status = sum6(check_bool(first3_array([true, false, false]), true), check_bool(first4_array([false, true, true, true]), false), check_bool(first5_array([true, false, true, false, true]), true), check_bool(last3_array([true, false, true]), true), check_bool(last4_array([true, true, false, false]), false), check_bool(last5_array([false, true, false, true, false]), false))
    let numeric_status = sum6(check_int(sum3_int_array([2, 3, 4]), 9), check_int(sum4_int_array([2, 3, 4, 5]), 14), check_int(sum5_int_array([2, 3, 4, 5, 6]), 20), check_int(product3_int_array([2, 3, 4]), 24), check_int(product4_int_array([2, 3, 4, 5]), 120), check_int(product5_int_array([2, 3, 4, 5, 6]), 720))
    let extrema_status = sum6(check_int(max3_int_array([3, 9, 5]), 9), check_int(max4_int_array([3, 9, 5, 7]), 9), check_int(max5_int_array([3, 9, 5, 7, 11]), 11), check_int(min3_int_array([3, 9, 5]), 3), check_int(min4_int_array([3, 9, 5, 7]), 3), check_int(min5_int_array([3, 9, 5, 7, 1]), 1))
    let bool_all_status = sum6(check_bool(all3_bool_array([true, true, true]), true), check_bool(all4_bool_array([true, true, true, false]), false), check_bool(all5_bool_array([true, true, true, true, true]), true), check_bool(any3_bool_array([false, false, true]), true), check_bool(any4_bool_array([false, false, false, false]), false), check_bool(any5_bool_array([false, false, false, false, true]), true))
    let bool_none_status = sum6(check_bool(none3_bool_array([false, false, false]), true), check_bool(none4_bool_array([false, false, true, false]), false), check_bool(none5_bool_array([false, false, false, false, false]), true), 0, 0, 0)

    return generic_int_status + generic_bool_status + numeric_status + extrema_status + bool_all_status + bool_none_status
}
