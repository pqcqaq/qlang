use std.array.all_bool_array as all_bool_array
use std.array.contains_array as contains_array
use std.array.repeat_array as repeat_array
use std.core.bool_to_int as bool_to_int
use std.option.Option as Option
use std.option.some as option_some
use std.option.unwrap_or as option_unwrap_or
use std.result.Result as Result
use std.result.ok as result_ok
use std.result.to_option as result_to_option
use std.result.unwrap_result_or as result_unwrap_result_or

fn main() -> Int {
    let repeated_false: [Bool; 3] = repeat_array(false)
    let enabled: Option[Bool] = option_some(true)
    let repeated_enabled: [Bool; 3] = [option_unwrap_or(enabled, false); 3]
    let all_enabled: Result[Bool, Int] = result_ok(all_bool_array(repeated_enabled))
    let result_value: Result[Bool, Int] = result_ok(option_unwrap_or(result_to_option(all_enabled), false) && contains_array(repeated_false, false))
    return 1 - bool_to_int(result_unwrap_result_or(result_value, false))
}
