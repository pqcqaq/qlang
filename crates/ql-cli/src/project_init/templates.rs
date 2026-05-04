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
use std.array.sum_int_array as sum_int_array
use std.core.clamp_int as clamp_int
use std.option.some as option_some
use std.option.unwrap_or as option_unwrap_or
use std.result.Result as Result
use std.result.ok as result_ok
use std.result.unwrap_result_or as result_unwrap_result_or

pub fn run() -> Int {
    let result_value: Result[Int, Int] = result_ok(option_unwrap_or(option_some(42), 0))
    let transformed_total = sum_int_array([1, first_array([2, 3, 4]), at_array_or([3, 4, 5], 1, 0)])
    let query_values: [Int; 3] = [3, 2, 1]
    let contains_bonus = if contains_array(query_values, 1) { 1 } else { 0 }
    return clamp_int(result_unwrap_result_or(result_value, 0) + transformed_total + sum_int_array([1, 1, 1]) + count_array([1, 2, 1], 1) + contains_bonus, 0, 100)
}
"#
}

pub(super) fn stdlib_package_main_source() -> &'static str {
    r#"use std.array.all_bool_array as all_bool_array
use std.array.contains_array as contains_array
use std.core.bool_to_int as bool_to_int
use std.option.Option as Option
use std.option.some as option_some
use std.option.unwrap_or as option_unwrap_or
use std.result.Result as Result
use std.result.ok as result_ok
use std.result.unwrap_result_or as result_unwrap_result_or

fn main() -> Int {
    let repeated_false: [Bool; 3] = [false, false, false]
    let enabled: Option[Bool] = option_some(true)
    let repeated_enabled: [Bool; 3] = [option_unwrap_or(enabled, false), true, true]
    let result_value: Result[Bool, Int] = result_ok(all_bool_array(repeated_enabled) && contains_array(repeated_false, false))
    return 1 - bool_to_int(result_unwrap_result_or(result_value, false))
}
"#
}

pub(super) fn stdlib_package_test_source() -> &'static str {
    r#"use std.array.contains_array as contains_array
use std.array.len_array as len_array
use std.array.sum_int_array as sum_int_array
use std.core.clamp_int as clamp_int
use std.option.Option as Option
use std.option.some as option_some
use std.option.unwrap_or as option_unwrap_or
use std.result.Result as Result
use std.result.ok as result_ok
use std.result.unwrap_result_or as result_unwrap_result_or
use std.test.expect_bool_eq as expect_bool_eq
use std.test.expect_int_eq as expect_int_eq
use std.test.expect_status_ok as expect_status_ok

fn main() -> Int {
    let numbers: [Int; 3] = [1, 2, 3]
    let option_value: Option[Int] = option_some(sum_int_array(numbers))
    let result_value: Result[Int, Int] = result_ok(option_unwrap_or(option_value, 0))
    let total = clamp_int(result_unwrap_result_or(result_value, 0), 0, 10)
    let total_check = expect_int_eq(total, 6)
    let length_check = expect_int_eq(len_array(numbers), 3)
    let contains_check = expect_bool_eq(contains_array(numbers, 2), true)
    return expect_status_ok(total_check + length_check + contains_check)
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
        assert!(lib.contains("use std.option.some as option_some"));
        assert!(lib.contains("use std.result.Result as Result"));
        assert!(lib.contains("use std.result.ok as result_ok"));
        assert!(lib.contains("use std.result.unwrap_result_or as result_unwrap_result_or"));
        assert!(lib.contains("use std.array.at_array_or"));
        assert!(lib.contains("use std.array.contains_array"));
        assert!(lib.contains("use std.array.count_array"));
        assert!(lib.contains("use std.array.first_array"));
        assert!(lib.contains("use std.array.sum_int_array"));
        assert!(main.contains("use std.core.bool_to_int"));
        assert!(main.contains("use std.option.Option as Option"));
        assert!(main.contains("use std.result.Result as Result"));
        assert!(main.contains("use std.array.all_bool_array"));
        assert!(main.contains("use std.array.contains_array"));
        assert!(!main.contains("some_bool"));
        assert!(!main.contains("ok_bool"));
        assert!(!lib.contains("repeat3_array"));
        assert!(!lib.contains("reverse3_array"));
        assert!(smoke.contains("use std.option.Option as Option"));
        assert!(smoke.contains("use std.result.Result as Result"));
        assert!(smoke.contains("use std.array.len_array as len_array"));
        assert!(smoke.contains("use std.array.sum_int_array as sum_int_array"));
        assert!(smoke.contains("use std.test.expect_int_eq as expect_int_eq"));
        assert!(smoke.contains("use std.test.expect_bool_eq as expect_bool_eq"));
        assert!(smoke.contains("use std.test.expect_status_ok as expect_status_ok"));
        assert!(smoke.contains("let numbers: [Int; 3] = [1, 2, 3]"));
        assert!(
            smoke.contains("let option_value: Option[Int] = option_some(sum_int_array(numbers))")
        );
        assert!(smoke.contains(
            "let result_value: Result[Int, Int] = result_ok(option_unwrap_or(option_value, 0))"
        ));
    }
}
