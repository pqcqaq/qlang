pub(crate) fn default_package_main_source() -> &'static str {
    "fn main() -> Int {\n    return 0\n}\n"
}

pub(super) fn default_package_source() -> &'static str {
    "pub fn run() -> Int {\n    return 0\n}\n"
}

pub(super) fn default_package_test_source() -> &'static str {
    "fn main() -> Int {\n    return 0\n}\n"
}

pub(super) fn stdlib_package_source() -> &'static str {
    r#"use std.array.at_array_or as at_array_or
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
"#
}

pub(super) fn stdlib_package_main_source() -> &'static str {
    r#"use std.array.all_bool_array as all_bool_array
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
"#
}

pub(super) fn stdlib_package_test_source() -> &'static str {
    r#"use std.array.contains_array as contains_array
use std.array.len_array as len_array
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
use std.test.expect_bool_eq as expect_bool_eq
use std.test.expect_int_eq as expect_int_eq
use std.test.expect_status_ok as expect_status_ok

fn main() -> Int {
    let numbers: [Int; 3] = [1, 2, 3]
    let repeated: [Int; 3] = repeat_array(2)
    let option_value: Option[Int] = option_some(sum_int_array(numbers))
    let missing: Option[Int] = Option.None
    let result_value: Result[Int, Int] = result_ok_or(option_value, 9)
    let failed: Result[Int, Int] = result_ok_or(missing, 4)
    let total = clamp_int(option_unwrap_or(result_to_option(result_value), 0), 0, 10)
    let total_check = expect_int_eq(total, 6)
    let length_check = expect_int_eq(len_array(numbers), 3)
    let contains_check = expect_bool_eq(contains_array(numbers, 2), true)
    let repeated_check = expect_int_eq(sum_int_array(repeated), 6)
    let error_check = expect_int_eq(option_unwrap_or(result_error_to_option(failed), 0), 4)
    return expect_status_ok(total_check + length_check + contains_check + repeated_check + error_check)
}
"#
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_templates_are_minimal_runnable_sources() {
        assert!(default_package_source().contains("pub fn run() -> Int"));
        assert_eq!(
            default_package_main_source(),
            "fn main() -> Int {\n    return 0\n}\n"
        );
        assert_eq!(
            default_package_test_source(),
            "fn main() -> Int {\n    return 0\n}\n"
        );
    }

    #[test]
    fn stdlib_templates_exercise_core_option_result_and_array_dependencies() {
        let lib = stdlib_package_source();
        let main = stdlib_package_main_source();
        let smoke = stdlib_package_test_source();

        assert!(lib.contains("use std.core.clamp_int"));
        assert!(lib.contains("use std.option.Option as Option"));
        assert!(lib.contains("use std.option.some as option_some"));
        assert!(lib.contains("use std.result.Result as Result"));
        assert!(lib.contains("use std.result.error_to_option as result_error_to_option"));
        assert!(lib.contains("use std.result.ok_or as result_ok_or"));
        assert!(lib.contains("use std.result.to_option as result_to_option"));
        assert!(lib.contains("use std.array.at_array_or"));
        assert!(lib.contains("use std.array.contains_array"));
        assert!(lib.contains("use std.array.count_array"));
        assert!(lib.contains("use std.array.first_array"));
        assert!(lib.contains("use std.array.repeat_array"));
        assert!(lib.contains("use std.array.sum_int_array"));
        assert!(lib.contains("let repeated: [Int; 3] = repeat_array(1)"));
        assert!(main.contains("use std.core.bool_to_int"));
        assert!(main.contains("use std.option.Option as Option"));
        assert!(main.contains("use std.result.Result as Result"));
        assert!(main.contains("use std.result.to_option as result_to_option"));
        assert!(main.contains("use std.array.all_bool_array"));
        assert!(main.contains("use std.array.contains_array"));
        assert!(main.contains("use std.array.repeat_array"));
        assert!(
            main.contains(
                "let repeated_enabled: [Bool; 3] = [option_unwrap_or(enabled, false); 3]"
            )
        );
        assert!(!main.contains("some_bool"));
        assert!(!main.contains("ok_bool"));
        assert!(!lib.contains("repeat3_array"));
        assert!(!lib.contains("reverse3_array"));
        assert!(!lib.contains("unwrap_result_or as result_unwrap_result_or"));
        assert!(smoke.contains("use std.option.Option as Option"));
        assert!(smoke.contains("use std.result.Result as Result"));
        assert!(smoke.contains("use std.result.error_to_option as result_error_to_option"));
        assert!(smoke.contains("use std.result.ok_or as result_ok_or"));
        assert!(smoke.contains("use std.result.to_option as result_to_option"));
        assert!(smoke.contains("use std.array.len_array as len_array"));
        assert!(smoke.contains("use std.array.repeat_array as repeat_array"));
        assert!(smoke.contains("use std.array.sum_int_array as sum_int_array"));
        assert!(smoke.contains("use std.test.expect_int_eq as expect_int_eq"));
        assert!(smoke.contains("use std.test.expect_bool_eq as expect_bool_eq"));
        assert!(smoke.contains("use std.test.expect_status_ok as expect_status_ok"));
        assert!(smoke.contains("let numbers: [Int; 3] = [1, 2, 3]"));
        assert!(smoke.contains("let repeated: [Int; 3] = repeat_array(2)"));
        assert!(
            smoke.contains("let option_value: Option[Int] = option_some(sum_int_array(numbers))")
        );
        assert!(
            smoke.contains("let result_value: Result[Int, Int] = result_ok_or(option_value, 9)")
        );
        assert!(smoke.contains("let failed: Result[Int, Int] = result_ok_or(missing, 4)"));
    }
}
