use std.result.Result as Result
use std.result.err as result_err
use std.result.error_or as result_error_or
use std.result.is_err as result_is_err
use std.result.is_ok as result_is_ok
use std.result.ok as result_ok
use std.result.or_result as result_or
use std.result.unwrap_result_or as result_unwrap_result_or

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

fn main() -> Int {
    let int_ok: Result[Int, Int] = result_ok(7)
    let int_err: Result[Int, Int] = result_err(3)
    let int_fallback: Result[Int, Int] = result_ok(11)
    let bool_ok: Result[Bool, Int] = result_ok(true)
    let bool_err: Result[Bool, Int] = result_err(4)
    let bool_fallback: Result[Bool, Int] = result_ok(false)
    let direct_ok: Result[Int, Int] = Result.Ok(21)
    let direct_err: Result[Int, Int] = Result.Err(6)

    let int_status = sum6(check_bool(result_is_ok(int_ok), true), check_bool(result_is_err(int_err), true), check_int(result_unwrap_result_or(int_ok, 0), 7), check_int(result_unwrap_result_or(int_err, 9), 9), check_int(result_error_or(int_ok, 0), 0), check_int(result_error_or(int_err, 0), 3))
    let bool_status = sum6(check_bool(result_is_ok(bool_ok), true), check_bool(result_is_err(bool_err), true), check_bool(result_unwrap_result_or(bool_ok, false), true), check_bool(result_unwrap_result_or(bool_err, true), true), check_int(result_error_or(bool_ok, 0), 0), check_int(result_error_or(bool_err, 0), 4))
    let constructor_status = sum6(check_int(result_unwrap_result_or(direct_ok, 0), 21), check_int(result_error_or(direct_err, 0), 6), check_bool(result_is_ok(bool_ok), true), check_bool(result_is_err(bool_err), true), 0, 0)

    return int_status + bool_status + constructor_status + check_int(result_unwrap_result_or(result_or(int_err, int_fallback), 0), 11) + check_bool(result_unwrap_result_or(result_or(bool_err, bool_fallback), true), false) + check_int(generic_result_status(Result.Ok(7), Result.Err(3)), 10)
}
