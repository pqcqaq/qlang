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
    r#"use std.array.sum3_int_array as sum3_int_array
use std.core.clamp_int as clamp_int
use std.option.some as option_some
use std.option.unwrap_or as option_unwrap_or
use std.result.Result as Result
use std.result.ok as result_ok
use std.result.unwrap_result_or as result_unwrap_result_or

pub fn run() -> Int {
    let result_value: Result[Int, Int] = result_ok(option_unwrap_or(option_some(42), 0))
    return clamp_int(result_unwrap_result_or(result_value, 0) + sum3_int_array([1, 2, 3]), 0, 100)
}
"#
}

pub(super) fn stdlib_package_main_source() -> &'static str {
    r#"use std.array.all3_bool_array as all3_bool_array
use std.core.bool_to_int as bool_to_int
use std.option.some_bool as some_bool
use std.option.unwrap_or_bool as unwrap_or_bool
use std.result.ok_bool as result_ok_bool
use std.result.unwrap_result_or_bool as result_unwrap_or_bool

fn main() -> Int {
    return 1 - bool_to_int(result_unwrap_or_bool(result_ok_bool(all3_bool_array([true, unwrap_or_bool(some_bool(true), false), true])), false))
}
"#
}

pub(super) fn stdlib_package_test_source() -> &'static str {
    super::super::stdlib_package_test_source()
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
        assert!(lib.contains("use std.array.sum3_int_array"));
        assert!(main.contains("use std.core.bool_to_int"));
        assert!(main.contains("use std.array.all3_bool_array"));
        assert!(smoke.contains("use std.test.expect_status_ok"));
        assert!(smoke.contains("use std.option.none_option as option_none"));
        assert!(smoke.contains("let generic_none_int: Option[Int] = option_none()"));
        assert!(smoke.contains("use std.test.expect_generic_int_option_some"));
        assert!(smoke.contains("use std.test.expect_generic_int_result_ok"));
        assert!(smoke.contains("use std.test.expect_generic_int_result_to_option_some"));
        assert!(smoke.contains("use std.test.expect_generic_int_option_ok_or"));
        assert!(smoke.contains("use std.array.max5_int_array"));
        assert!(smoke.contains("use std.option.Option as Option"));
        assert!(smoke.contains("use std.result.Result as Result"));
    }
}
