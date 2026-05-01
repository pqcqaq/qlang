use std.result.err_bool as err_bool
use std.result.err_int as err_int
use std.result.error_or_zero_bool as error_or_zero_bool
use std.result.error_or_zero_int as error_or_zero_int
use std.result.is_err_bool as is_err_bool
use std.result.is_err_int as is_err_int
use std.result.is_ok_bool as is_ok_bool
use std.result.is_ok_int as is_ok_int
use std.result.ok_or_bool as ok_or_bool
use std.result.ok_or_int as ok_or_int
use std.result.ok_bool as ok_bool
use std.result.ok_int as ok_int
use std.result.or_result_bool as or_result_bool
use std.result.or_result_int as or_result_int
use std.result.to_option_bool as to_option_bool
use std.result.to_option_int as to_option_int
use std.result.unwrap_result_or_bool as unwrap_result_or_bool
use std.result.unwrap_result_or_int as unwrap_result_or_int
use std.option.is_none_bool as option_is_none_bool
use std.option.is_none_int as option_is_none_int
use std.option.none_bool as option_none_bool
use std.option.none_int as option_none_int
use std.option.some_bool as option_some_bool
use std.option.some_int as option_some_int
use std.option.unwrap_or_bool as option_unwrap_or_bool
use std.option.unwrap_or_int as option_unwrap_or_int

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
    let int_status = sum6(check_bool(is_ok_int(ok_int(7)), true), check_bool(is_err_int(err_int(3)), true), check_int(unwrap_result_or_int(ok_int(7), 0), 7), check_int(unwrap_result_or_int(err_int(3), 9), 9), check_int(error_or_zero_int(ok_int(7)), 0), check_int(error_or_zero_int(err_int(3)), 3))
    let bool_status = sum6(check_bool(is_ok_bool(ok_bool(true)), true), check_bool(is_err_bool(err_bool(4)), true), check_bool(unwrap_result_or_bool(ok_bool(true), false), true), check_bool(unwrap_result_or_bool(err_bool(4), true), true), check_int(error_or_zero_bool(ok_bool(false)), 0), check_int(error_or_zero_bool(err_bool(4)), 4))
    let option_status = sum6(check_int(option_unwrap_or_int(to_option_int(ok_int(13)), 0), 13), check_bool(option_is_none_int(to_option_int(err_int(3))), true), check_bool(option_unwrap_or_bool(to_option_bool(ok_bool(false)), true), false), check_bool(option_is_none_bool(to_option_bool(err_bool(4))), true), check_int(unwrap_result_or_int(ok_or_int(option_some_int(19), 5), 0), 19), check_int(error_or_zero_int(ok_or_int(option_none_int(), 5)), 5))
    let option_bool_status = sum6(check_bool(unwrap_result_or_bool(ok_or_bool(option_some_bool(true), 6), false), true), check_int(error_or_zero_bool(ok_or_bool(option_none_bool(), 6)), 6), 0, 0, 0, 0)

    return int_status + bool_status + option_status + option_bool_status + check_int(unwrap_result_or_int(or_result_int(err_int(5), ok_int(11)), 0), 11) + check_bool(unwrap_result_or_bool(or_result_bool(err_bool(6), ok_bool(false)), true), false)
}
