use std.result.err_bool as err_bool
use std.result.err_int as err_int
use std.result.error_to_option_bool as error_to_option_bool
use std.result.error_to_option_int as error_to_option_int
use std.result.error_or as result_error_or
use std.result.error_or_zero_bool as error_or_zero_bool
use std.result.error_or_zero_int as error_or_zero_int
use std.result.err as result_err
use std.result.is_err as result_is_err
use std.result.is_err_bool as is_err_bool
use std.result.is_err_int as is_err_int
use std.result.ok as result_ok
use std.result.ok_or as result_ok_or
use std.result.is_ok as result_is_ok
use std.result.is_ok_bool as is_ok_bool
use std.result.is_ok_int as is_ok_int
use std.result.ok_or_bool as ok_or_bool
use std.result.ok_or_int as ok_or_int
use std.result.Result as Result
use std.result.ok_bool as ok_bool
use std.result.ok_int as ok_int
use std.result.or_result as generic_or_result
use std.result.or_result_bool as or_result_bool
use std.result.or_result_int as or_result_int
use std.result.error_to_option as result_error_to_option
use std.result.to_option as result_to_option
use std.result.to_option_bool as to_option_bool
use std.result.to_option_int as to_option_int
use std.result.unwrap_result_or as result_unwrap_result_or
use std.result.unwrap_result_or_bool as unwrap_result_or_bool
use std.result.unwrap_result_or_int as unwrap_result_or_int
use std.option.Option as Option
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

fn generic_result_status(ok_value: Result[Int, Int], err_value: Result[Int, Int]) -> Int {
    let ok_status = match ok_value {
        Result.Ok(inner) => inner,
        Result.Err(_) => 0,
    }
    let err_status = match err_value {
        Result.Ok(_) => 1,
        Result.Err(error) => error,
    }
    return ok_status + err_status
}

fn generic_option_is_none(value: Option[Int]) -> Bool {
    return match value {
        Option.Some(_) => false,
        Option.None => true,
    }
}

fn generic_option_value_or(value: Option[Int], fallback: Int) -> Int {
    return match value {
        Option.Some(inner) => inner,
        Option.None => fallback,
    }
}

fn main() -> Int {
    let int_status = sum6(check_bool(is_ok_int(ok_int(7)), true), check_bool(is_err_int(err_int(3)), true), check_int(unwrap_result_or_int(ok_int(7), 0), 7), check_int(unwrap_result_or_int(err_int(3), 9), 9), check_int(error_or_zero_int(ok_int(7)), 0), check_int(error_or_zero_int(err_int(3)), 3))
    let bool_status = sum6(check_bool(is_ok_bool(ok_bool(true)), true), check_bool(is_err_bool(err_bool(4)), true), check_bool(unwrap_result_or_bool(ok_bool(true), false), true), check_bool(unwrap_result_or_bool(err_bool(4), true), true), check_int(error_or_zero_bool(ok_bool(false)), 0), check_int(error_or_zero_bool(err_bool(4)), 4))
    let option_status = sum6(check_int(option_unwrap_or_int(to_option_int(ok_int(13)), 0), 13), check_bool(option_is_none_int(to_option_int(err_int(3))), true), check_bool(option_unwrap_or_bool(to_option_bool(ok_bool(false)), true), false), check_bool(option_is_none_bool(to_option_bool(err_bool(4))), true), check_int(unwrap_result_or_int(ok_or_int(option_some_int(19), 5), 0), 19), check_int(error_or_zero_int(ok_or_int(option_none_int(), 5)), 5))
    let option_bool_status = sum6(check_bool(unwrap_result_or_bool(ok_or_bool(option_some_bool(true), 6), false), true), check_int(error_or_zero_bool(ok_or_bool(option_none_bool(), 6)), 6), 0, 0, 0, 0)
    let error_option_status = sum6(check_int(option_unwrap_or_int(error_to_option_int(err_int(0)), 9), 0), check_bool(option_is_none_int(error_to_option_int(ok_int(7))), true), check_int(option_unwrap_or_int(error_to_option_bool(err_bool(0)), 9), 0), check_bool(option_is_none_int(error_to_option_bool(ok_bool(false))), true), 0, 0)
    let generic_ok: Result[Int, Int] = result_ok(17)
    let generic_err: Result[Int, Int] = result_err(4)
    let generic_fallback: Result[Int, Int] = result_ok(23)
    let generic_or: Result[Int, Int] = generic_or_result(generic_err, generic_fallback)
    let generic_status = sum6(check_bool(result_is_ok(generic_ok), true), check_bool(result_is_err(generic_err), true), check_int(result_unwrap_result_or(generic_ok, 0), 17), check_int(result_unwrap_result_or(generic_err, 9), 9), check_int(result_error_or(generic_err, 0), 4), check_int(result_unwrap_result_or(generic_or, 0), 23))
    let generic_ok_or_some: Result[Int, Int] = result_ok_or(Option.Some(19), 5)
    let generic_none: Option[Int] = Option.None
    let generic_ok_or_none: Result[Int, Int] = result_ok_or(generic_none, 5)
    let generic_some_result: Result[Int, Int] = Result.Ok(21)
    let generic_err_result: Result[Int, Int] = Result.Err(6)
    let generic_conversion_status = sum6(check_int(result_unwrap_result_or(generic_ok_or_some, 0), 19), check_int(result_error_or(generic_ok_or_none, 0), 5), check_int(generic_option_value_or(result_to_option(generic_some_result), 0), 21), check_bool(generic_option_is_none(result_to_option(generic_err_result)), true), check_int(generic_option_value_or(result_error_to_option(generic_err_result), 0), 6), check_bool(generic_option_is_none(result_error_to_option(generic_some_result)), true))

    return int_status + bool_status + option_status + option_bool_status + error_option_status + generic_status + generic_conversion_status + check_int(unwrap_result_or_int(or_result_int(err_int(5), ok_int(11)), 0), 11) + check_bool(unwrap_result_or_bool(or_result_bool(err_bool(6), ok_bool(false)), true), false) + check_int(generic_result_status(Result.Ok(7), Result.Err(3)), 10)
}
