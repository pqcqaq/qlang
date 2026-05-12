use std::fs;
use std::path::Path;

use super::normalize_path;

pub(super) struct PackageSources {
    pub(super) package_source: String,
    pub(super) main_source: String,
    pub(super) test_source: String,
}

pub(crate) fn default_package_main_source() -> &'static str {
    "fn main() -> Int {\n    return 0\n}\n"
}

pub(super) fn default_package_source() -> &'static str {
    "pub fn run() -> Int {\n    return 0\n}\n"
}

pub(super) fn default_package_test_source() -> &'static str {
    "fn main() -> Int {\n    return 0\n}\n"
}

pub(super) fn default_package_sources() -> PackageSources {
    PackageSources {
        package_source: default_package_source().to_owned(),
        main_source: default_package_main_source().to_owned(),
        test_source: default_package_test_source().to_owned(),
    }
}

pub(super) fn stdlib_package_sources(stdlib_root: &Path) -> Result<PackageSources, String> {
    let starter_root = stdlib_root.join("examples").join("starter");
    Ok(PackageSources {
        package_source: read_starter_source(&starter_root.join("src").join("lib.ql"))?,
        main_source: read_starter_source(&starter_root.join("src").join("main.ql"))?,
        test_source: read_starter_source(&starter_root.join("tests").join("smoke.ql"))?,
    })
}

fn read_starter_source(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|error| {
        format!(
            "stdlib starter template `{}` is not available: {error}",
            normalize_path(path)
        )
    })
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
        let stdlib_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../stdlib");
        let sources = stdlib_package_sources(&stdlib_root)
            .expect("repo stdlib starter template should be readable");
        let lib = sources.package_source;
        let main = sources.main_source;
        let smoke = sources.test_source;

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
        assert!(smoke.contains("use std.result.ok_or as result_ok_or"));
        assert!(smoke.contains("use std.result.to_option as result_to_option"));
        assert!(smoke.contains("use std.array.len_array as len_array"));
        assert!(smoke.contains("use std.array.repeat_array as repeat_array"));
        assert!(smoke.contains("use std.array.sum_int_array as sum_int_array"));
        assert!(smoke.contains("use std.test.expect_eq as expect_eq"));
        assert!(smoke.contains("use std.test.expect_option_none as expect_option_none"));
        assert!(smoke.contains("use std.test.expect_option_some as expect_option_some"));
        assert!(smoke.contains("use std.test.expect_result_err as expect_result_err"));
        assert!(smoke.contains("use std.test.expect_result_ok as expect_result_ok"));
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
        assert!(smoke.contains(
            "let option_check = expect_option_some(option_value, 6) + expect_option_none(missing)"
        ));
        assert!(smoke.contains(
            "let result_check = expect_result_ok(result_value, 6) + expect_result_err(failed, 4)"
        ));
        assert!(!smoke.contains("result_error_to_option"));
    }
}
