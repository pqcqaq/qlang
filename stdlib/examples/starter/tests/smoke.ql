use std.array.contains_array as contains_array
use std.array.len_array as len_array
use std.array.repeat_array as repeat_array
use std.array.sum_int_array as sum_int_array
use std.core.clamp_int as clamp_int
use std.option.Option as Option
use std.option.some as option_some
use std.option.unwrap_or as option_unwrap_or
use std.result.Result as Result
use std.result.ok_or as result_ok_or
use std.result.to_option as result_to_option
use std.test.expect_array_eq as expect_array_eq
use std.test.expect_array_reverse as expect_array_reverse
use std.test.expect_eq as expect_eq
use std.test.expect_option_none as expect_option_none
use std.test.expect_option_some as expect_option_some
use std.test.expect_result_err as expect_result_err
use std.test.expect_result_ok as expect_result_ok

fn main() -> Int {
    let numbers: [Int; 3] = [1, 2, 3]
    let repeated: [Int; 3] = repeat_array(2)
    let option_value: Option[Int] = option_some(sum_int_array(numbers))
    let missing: Option[Int] = Option.None
    let result_value: Result[Int, Int] = result_ok_or(option_value, 9)
    let failed: Result[Int, Int] = result_ok_or(missing, 4)
    let total = clamp_int(option_unwrap_or(result_to_option(result_value), 0), 0, 10)
    let total_check = expect_eq(total, 6)
    let length_check = expect_eq(len_array(numbers), 3)
    let contains_check = expect_eq(contains_array(numbers, 2), true)
    let repeated_check = expect_eq(sum_int_array(repeated), 6)
    let array_check = expect_array_eq(repeated, [2, 2, 2]) + expect_array_reverse(numbers, [3, 2, 1])
    let option_check = expect_option_some(option_value, 6) + expect_option_none(missing)
    let result_check = expect_result_ok(result_value, 6) + expect_result_err(failed, 4)
    return expect_eq(total_check + length_check + contains_check + repeated_check + array_check + option_check + result_check, 0)
}
