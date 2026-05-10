use std.array.at_array_or as at_array_or
use std.array.contains_array as contains_array
use std.array.count_array as count_array
use std.array.first_array as first_array
use std.array.repeat_array as repeat_array
use std.array.sum_int_array as sum_int_array
use std.core.clamp_int as clamp_int
use std.option.Option as Option
use std.option.some as option_some
use std.option.unwrap_or as option_unwrap_or
use std.result.Result as Result
use std.result.error_to_option as result_error_to_option
use std.result.ok_or as result_ok_or
use std.result.to_option as result_to_option

pub fn run() -> Int {
    let option_value: Option[Int] = option_some(42)
    let result_value: Result[Int, Int] = result_ok_or(option_value, 5)
    let missing: Option[Int] = Option.None
    let failed: Result[Int, Int] = result_ok_or(missing, 7)
    let transformed_total = sum_int_array([1, first_array([2, 3, 4]), at_array_or([3, 4, 5], 1, 0)])
    let query_values: [Int; 3] = [3, 2, 1]
    let repeated: [Int; 3] = repeat_array(1)
    let contains_bonus = if contains_array(query_values, 1) { 1 } else { 0 }
    let option_total = option_unwrap_or(result_to_option(result_value), 0)
    let error_bonus = option_unwrap_or(result_error_to_option(failed), 0)
    return clamp_int(option_total + transformed_total + sum_int_array(repeated) + count_array([1, 2, 1], 1) + contains_bonus + error_bonus, 0, 100)
}
