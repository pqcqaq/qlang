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
use std.option.some_int as some_int
use std.option.unwrap_or_int as unwrap_or_int
use std.result.ok_int as result_ok_int
use std.result.unwrap_result_or_int as result_unwrap_or_int

pub fn run() -> Int {
    return clamp_int(result_unwrap_or_int(result_ok_int(unwrap_or_int(some_int(42), 0)), 0) + sum3_int_array([1, 2, 3]), 0, 100)
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
        assert!(lib.contains("use std.option.some_int"));
        assert!(lib.contains("use std.result.ok_int"));
        assert!(lib.contains("use std.array.sum3_int_array"));
        assert!(main.contains("use std.core.bool_to_int"));
        assert!(main.contains("use std.array.all3_bool_array"));
        assert!(smoke.contains("use std.test.expect_status_ok"));
        assert!(smoke.contains("use std.array.max5_int_array"));
    }
}
