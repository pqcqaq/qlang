mod support;

use std::fs;
use std::path::{Path, PathBuf};

use ql_driver::{ToolchainOptions, discover_toolchain};
use support::{
    TempDir, executable_output_path, expect_empty_stderr, expect_empty_stdout, expect_exit_code,
    expect_file_exists, expect_silent_output, expect_stderr_contains, expect_stdout_contains_all,
    expect_success, ql_command, read_normalized_file, run_command_capture, workspace_root,
};

fn toolchain_available(context: &str) -> bool {
    let Ok(_toolchain) = discover_toolchain(&ToolchainOptions::default()) else {
        eprintln!(
            "skipping {context}: no clang-style compiler found via ql-driver toolchain discovery"
        );
        return false;
    };
    true
}

fn write_repo_stdlib_fixture(temp: &TempDir, repo_root: &Path) -> PathBuf {
    let source_root = repo_root.join("stdlib");
    for relative in [
        "qlang.toml",
        "packages/core/qlang.toml",
        "packages/core/src/lib.ql",
        "packages/core/tests/smoke.ql",
        "packages/array/qlang.toml",
        "packages/array/src/lib.ql",
        "packages/array/tests/smoke.ql",
        "packages/option/qlang.toml",
        "packages/option/src/lib.ql",
        "packages/option/tests/smoke.ql",
        "packages/result/qlang.toml",
        "packages/result/src/lib.ql",
        "packages/result/tests/smoke.ql",
        "packages/test/qlang.toml",
        "packages/test/src/lib.ql",
        "packages/test/tests/smoke.ql",
    ] {
        let source_path = source_root.join(relative);
        let contents = fs::read_to_string(&source_path).unwrap_or_else(|error| {
            panic!("read stdlib fixture `{}`: {error}", source_path.display())
        });
        temp.write(&format!("stdlib/{relative}"), &contents);
    }
    temp.path().join("stdlib")
}

fn expected_stdlib_package_smoke_source() -> &'static str {
    r#"use std.core.abs_diff_int as abs_diff_int
use std.core.abs_int as abs_int
use std.core.all3_bool as all3_bool
use std.core.all4_bool as all4_bool
use std.core.all5_bool as all5_bool
use std.core.and_bool as and_bool
use std.core.any3_bool as any3_bool
use std.core.any4_bool as any4_bool
use std.core.any5_bool as any5_bool
use std.core.average2_int as average2_int
use std.core.average3_int as average3_int
use std.core.average4_int as average4_int
use std.core.average5_int as average5_int
use std.core.clamp_bounds_int as clamp_bounds_int
use std.core.clamp_max_int as clamp_max_int
use std.core.clamp_min_int as clamp_min_int
use std.core.compare_int as compare_int
use std.core.distance_to_bounds_int as distance_to_bounds_int
use std.core.distance_to_range_int as distance_to_range_int
use std.core.has_remainder_int as has_remainder_int
use std.core.implies_bool as implies_bool
use std.core.in_bounds_int as in_bounds_int
use std.core.in_exclusive_bounds_int as in_exclusive_bounds_int
use std.core.is_ascending_int as is_ascending_int
use std.core.is_descending_int as is_descending_int
use std.core.is_descending4_int as is_descending4_int
use std.core.is_descending5_int as is_descending5_int
use std.core.is_factor_of_int as is_factor_of_int
use std.core.is_not_within_int as is_not_within_int
use std.core.is_outside_bounds_int as is_outside_bounds_int
use std.core.is_outside_range_int as is_outside_range_int
use std.core.is_strictly_descending_int as is_strictly_descending_int
use std.core.is_strictly_ascending_int as is_strictly_ascending_int
use std.core.is_ascending4_int as is_ascending4_int
use std.core.is_ascending5_int as is_ascending5_int
use std.core.is_strictly_ascending4_int as is_strictly_ascending4_int
use std.core.is_strictly_ascending5_int as is_strictly_ascending5_int
use std.core.is_strictly_descending4_int as is_strictly_descending4_int
use std.core.is_strictly_descending5_int as is_strictly_descending5_int
use std.core.is_within_int as is_within_int
use std.core.lower_bound_int as lower_bound_int
use std.core.max3_int as max3_int
use std.core.max4_int as max4_int
use std.core.max5_int as max5_int
use std.core.max_int as max_int
use std.core.median3_int as median3_int
use std.core.min3_int as min3_int
use std.core.min4_int as min4_int
use std.core.min5_int as min5_int
use std.core.min_int as min_int
use std.core.none3_bool as none3_bool
use std.core.none4_bool as none4_bool
use std.core.none5_bool as none5_bool
use std.core.product3_int as product3_int
use std.core.product4_int as product4_int
use std.core.product5_int as product5_int
use std.core.quotient_or_zero_int as quotient_or_zero_int
use std.core.range_span_int as range_span_int
use std.core.remainder_or_zero_int as remainder_or_zero_int
use std.core.sign_int as sign_int
use std.core.sum3_int as sum3_int
use std.core.sum4_int as sum4_int
use std.core.sum5_int as sum5_int
use std.core.upper_bound_int as upper_bound_int
use std.core.xor_bool as xor_bool
use std.option.Option as Option
use std.option.none_option as option_none
use std.option.none_bool as none_bool
use std.option.none_int as none_int
use std.option.some_bool as some_bool
use std.option.some_int as some_int
use std.result.Result as Result
use std.result.err_bool as result_err_bool
use std.result.err_int as result_err_int
use std.result.ok_bool as result_ok_bool
use std.result.ok_int as result_ok_int
use std.test.expect_bool_all3 as expect_bool_all3
use std.test.expect_bool_all4 as expect_bool_all4
use std.test.expect_bool_all5 as expect_bool_all5
use std.test.expect_bool_and as expect_bool_and
use std.test.expect_bool_array_all3 as expect_bool_array_all3
use std.test.expect_bool_array_any5 as expect_bool_array_any5
use std.test.expect_bool_array_last5 as expect_bool_array_last5
use std.test.expect_bool_array_none4 as expect_bool_array_none4
use std.test.expect_bool_any3 as expect_bool_any3
use std.test.expect_bool_any4 as expect_bool_any4
use std.test.expect_bool_any5 as expect_bool_any5
use std.test.expect_bool_eq as expect_bool_eq
use std.test.expect_bool_implies as expect_bool_implies
use std.test.expect_bool_ne as expect_bool_ne
use std.test.expect_bool_none3 as expect_bool_none3
use std.test.expect_bool_none4 as expect_bool_none4
use std.test.expect_bool_none5 as expect_bool_none5
use std.test.expect_bool_not as expect_bool_not
use std.test.expect_bool_option_none as expect_bool_option_none
use std.test.expect_bool_option_ok_or as expect_bool_option_ok_or
use std.test.expect_bool_option_ok_or_err as expect_bool_option_ok_or_err
use std.test.expect_bool_option_or as expect_bool_option_or
use std.test.expect_bool_option_some as expect_bool_option_some
use std.test.expect_bool_or as expect_bool_or
use std.test.expect_bool_result_err as expect_bool_result_err
use std.test.expect_bool_result_error_none as expect_bool_result_error_none
use std.test.expect_bool_result_error_some as expect_bool_result_error_some
use std.test.expect_bool_result_ok as expect_bool_result_ok
use std.test.expect_bool_result_or as expect_bool_result_or
use std.test.expect_bool_result_to_option_none as expect_bool_result_to_option_none
use std.test.expect_bool_result_to_option_some as expect_bool_result_to_option_some
use std.test.expect_bool_to_int as expect_bool_to_int
use std.test.expect_bool_xor as expect_bool_xor
use std.test.expect_false as expect_false
use std.test.expect_generic_bool_option_none as expect_generic_bool_option_none
use std.test.expect_generic_bool_option_ok_or as expect_generic_bool_option_ok_or
use std.test.expect_generic_bool_option_ok_or_err as expect_generic_bool_option_ok_or_err
use std.test.expect_generic_bool_option_or as expect_generic_bool_option_or
use std.test.expect_generic_bool_option_some as expect_generic_bool_option_some
use std.test.expect_generic_bool_result_err as expect_generic_bool_result_err
use std.test.expect_generic_bool_result_error as expect_generic_bool_result_error
use std.test.expect_generic_bool_result_error_none as expect_generic_bool_result_error_none
use std.test.expect_generic_bool_result_error_some as expect_generic_bool_result_error_some
use std.test.expect_generic_bool_result_ok as expect_generic_bool_result_ok
use std.test.expect_generic_bool_result_or as expect_generic_bool_result_or
use std.test.expect_generic_bool_result_to_option_none as expect_generic_bool_result_to_option_none
use std.test.expect_generic_bool_result_to_option_some as expect_generic_bool_result_to_option_some
use std.test.expect_generic_int_option_none as expect_generic_int_option_none
use std.test.expect_generic_int_option_ok_or as expect_generic_int_option_ok_or
use std.test.expect_generic_int_option_ok_or_err as expect_generic_int_option_ok_or_err
use std.test.expect_generic_int_option_or as expect_generic_int_option_or
use std.test.expect_generic_int_option_some as expect_generic_int_option_some
use std.test.expect_generic_int_result_err as expect_generic_int_result_err
use std.test.expect_generic_int_result_error as expect_generic_int_result_error
use std.test.expect_generic_int_result_error_none as expect_generic_int_result_error_none
use std.test.expect_generic_int_result_error_some as expect_generic_int_result_error_some
use std.test.expect_generic_int_result_ok as expect_generic_int_result_ok
use std.test.expect_generic_int_result_or as expect_generic_int_result_or
use std.test.expect_generic_int_result_to_option_none as expect_generic_int_result_to_option_none
use std.test.expect_generic_int_result_to_option_some as expect_generic_int_result_to_option_some
use std.test.expect_int_abs as expect_int_abs
use std.test.expect_int_abs_diff as expect_int_abs_diff
use std.test.expect_int_array_first3 as expect_int_array_first3
use std.test.expect_int_array_max5 as expect_int_array_max5
use std.test.expect_int_array_min5 as expect_int_array_min5
use std.test.expect_int_array_product4 as expect_int_array_product4
use std.test.expect_int_array_sum3 as expect_int_array_sum3
use std.test.expect_int_array_sum5 as expect_int_array_sum5
use std.test.expect_status_failed as expect_status_failed
use std.test.expect_status_ok as expect_status_ok
use std.test.expect_int_average2 as expect_int_average2
use std.test.expect_int_average3 as expect_int_average3
use std.test.expect_int_average4 as expect_int_average4
use std.test.expect_int_average5 as expect_int_average5
use std.test.expect_int_ascending as expect_int_ascending
use std.test.expect_int_ascending4 as expect_int_ascending4
use std.test.expect_int_ascending5 as expect_int_ascending5
use std.test.expect_int_between as expect_int_between
use std.test.expect_int_between_bounds as expect_int_between_bounds
use std.test.expect_int_clamp_max as expect_int_clamp_max
use std.test.expect_int_clamp_min as expect_int_clamp_min
use std.test.expect_int_clamped as expect_int_clamped
use std.test.expect_int_clamped_bounds as expect_int_clamped_bounds
use std.test.expect_int_compare as expect_int_compare
use std.test.expect_int_descending as expect_int_descending
use std.test.expect_int_descending4 as expect_int_descending4
use std.test.expect_int_descending5 as expect_int_descending5
use std.test.expect_int_distance_to_bounds as expect_int_distance_to_bounds
use std.test.expect_int_distance_to_range as expect_int_distance_to_range
use std.test.expect_int_divisible_by as expect_int_divisible_by
use std.test.expect_int_eq as expect_int_eq
use std.test.expect_int_even as expect_int_even
use std.test.expect_int_exclusive_between_bounds as expect_int_exclusive_between_bounds
use std.test.expect_int_exclusive_between as expect_int_exclusive_between
use std.test.expect_int_factor_of as expect_int_factor_of
use std.test.expect_int_option_none as expect_int_option_none
use std.test.expect_int_option_ok_or as expect_int_option_ok_or
use std.test.expect_int_option_ok_or_err as expect_int_option_ok_or_err
use std.test.expect_int_option_or as expect_int_option_or
use std.test.expect_int_option_some as expect_int_option_some
use std.test.expect_int_result_err as expect_int_result_err
use std.test.expect_int_result_error_none as expect_int_result_error_none
use std.test.expect_int_result_error_some as expect_int_result_error_some
use std.test.expect_int_result_ok as expect_int_result_ok
use std.test.expect_int_result_or as expect_int_result_or
use std.test.expect_int_result_to_option_none as expect_int_result_to_option_none
use std.test.expect_int_result_to_option_some as expect_int_result_to_option_some
use std.test.expect_int_max as expect_int_max
use std.test.expect_int_max3 as expect_int_max3
use std.test.expect_int_max4 as expect_int_max4
use std.test.expect_int_max5 as expect_int_max5
use std.test.expect_int_median3 as expect_int_median3
use std.test.expect_int_min as expect_int_min
use std.test.expect_int_min3 as expect_int_min3
use std.test.expect_int_min4 as expect_int_min4
use std.test.expect_int_min5 as expect_int_min5
use std.test.expect_int_has_remainder as expect_int_has_remainder
use std.test.expect_int_negative as expect_int_negative
use std.test.expect_int_not_within as expect_int_not_within
use std.test.expect_int_nonnegative as expect_int_nonnegative
use std.test.expect_int_nonpositive as expect_int_nonpositive
use std.test.expect_int_lower_bound as expect_int_lower_bound
use std.test.expect_int_odd as expect_int_odd
use std.test.expect_int_outside as expect_int_outside
use std.test.expect_int_outside_bounds as expect_int_outside_bounds
use std.test.expect_int_positive as expect_int_positive
use std.test.expect_int_product3 as expect_int_product3
use std.test.expect_int_product4 as expect_int_product4
use std.test.expect_int_product5 as expect_int_product5
use std.test.expect_int_quotient_or_zero as expect_int_quotient_or_zero
use std.test.expect_int_range_span as expect_int_range_span
use std.test.expect_int_remainder_or_zero as expect_int_remainder_or_zero
use std.test.expect_int_sign as expect_int_sign
use std.test.expect_int_strictly_descending as expect_int_strictly_descending
use std.test.expect_int_strictly_ascending as expect_int_strictly_ascending
use std.test.expect_int_strictly_ascending4 as expect_int_strictly_ascending4
use std.test.expect_int_strictly_ascending5 as expect_int_strictly_ascending5
use std.test.expect_int_strictly_descending4 as expect_int_strictly_descending4
use std.test.expect_int_strictly_descending5 as expect_int_strictly_descending5
use std.test.expect_int_sum3 as expect_int_sum3
use std.test.expect_int_sum4 as expect_int_sum4
use std.test.expect_int_sum5 as expect_int_sum5
use std.test.expect_int_upper_bound as expect_int_upper_bound
use std.test.expect_int_within as expect_int_within
use std.test.expect_true as expect_true
use std.test.is_status_failed as is_status_failed
use std.test.is_status_ok as is_status_ok
use std.test.merge_status as merge_status
use std.test.merge_status3 as merge_status3
use std.test.merge_status4 as merge_status4
use std.test.merge_status5 as merge_status5
use std.test.merge_status6 as merge_status6

fn main() -> Int {
    let max_check = expect_int_eq(max_int(20, 22), 22)
    let max3_check = expect_int_eq(max3_int(20, 22, 21), 22)
    let max4_check = expect_int_eq(max4_int(20, 22, 21, 19), 22)
    let max5_check = expect_int_eq(max5_int(20, 22, 21, 19, 23), 23)
    let min_check = expect_int_eq(min_int(20, 22), 20)
    let min3_check = expect_int_eq(min3_int(20, 22, 21), 20)
    let min4_check = expect_int_eq(min4_int(20, 22, 21, 19), 19)
    let min5_check = expect_int_eq(min5_int(20, 22, 21, 19, 18), 18)
    let sum3_check = expect_int_eq(sum3_int(2, 3, 4), 9)
    let sum4_check = expect_int_eq(sum4_int(2, 3, 4, 5), 14)
    let sum5_check = expect_int_eq(sum5_int(2, 3, 4, 5, 6), 20)
    let product3_check = expect_int_eq(product3_int(2, 3, 4), 24)
    let product4_check = expect_int_eq(product4_int(2, 3, 4, 5), 120)
    let product5_check = expect_int_eq(product5_int(2, 3, 4, 5, 6), 720)
    let average2_check = expect_int_eq(average2_int(5, 8), 6)
    let average3_check = expect_int_eq(average3_int(3, 6, 9), 6)
    let average4_check = expect_int_eq(average4_int(2, 4, 6, 8), 5)
    let average5_check = expect_int_eq(average5_int(2, 4, 6, 8, 10), 6)
    let quotient_check = expect_int_eq(quotient_or_zero_int(21, 7), 3)
    let quotient_zero_check = expect_int_eq(quotient_or_zero_int(21, 0), 0)
    let remainder_check = expect_int_eq(remainder_or_zero_int(22, 7), 1)
    let remainder_zero_check = expect_int_eq(remainder_or_zero_int(22, 0), 0)
    let has_remainder_check = expect_bool_eq(has_remainder_int(22, 7), true)
    let factor_check = expect_bool_eq(is_factor_of_int(7, 21), true)
    let median3_check = expect_int_eq(median3_int(22, 20, 21), 21)
    let clamp_min_check = expect_int_eq(clamp_min_int(19, 20), 20)
    let clamp_max_check = expect_int_eq(clamp_max_int(23, 22), 22)
    let clamp_bounds_check = expect_int_eq(clamp_bounds_int(23, 22, 20), 22)
    let abs_check = expect_int_eq(abs_int(0 - 22), 22)
    let abs_diff_check = expect_int_eq(abs_diff_int(22, 19), 3)
    let range_span_check = expect_int_eq(range_span_int(22, 20), 2)
    let lower_bound_check = expect_int_eq(lower_bound_int(22, 20), 20)
    let upper_bound_check = expect_int_eq(upper_bound_int(22, 20), 22)
    let distance_range_check = expect_int_eq(distance_to_range_int(19, 20, 22), 1)
    let distance_bounds_check = expect_int_eq(distance_to_bounds_int(23, 22, 20), 1)
    let compare_check = expect_int_eq(compare_int(9, 3), 1)
    let sign_negative_check = expect_int_eq(sign_int(0 - 5), 0 - 1)
    let sign_zero_check = expect_int_eq(sign_int(0), 0)
    let sign_positive_check = expect_int_eq(sign_int(5), 1)
    let and_check = expect_false(and_bool(true, false))
    let xor_check = expect_bool_eq(xor_bool(true, false), true)
    let all3_check = expect_bool_eq(all3_bool(true, true, true), true)
    let all4_check = expect_bool_eq(all4_bool(true, true, true, false), false)
    let all5_check = expect_bool_eq(all5_bool(true, true, true, true, true), true)
    let any3_check = expect_bool_eq(any3_bool(false, false, true), true)
    let any4_check = expect_bool_eq(any4_bool(false, false, false, false), false)
    let any5_check = expect_bool_eq(any5_bool(false, false, false, false, true), true)
    let none3_check = expect_bool_eq(none3_bool(false, false, false), true)
    let none4_check = expect_bool_eq(none4_bool(false, false, true, false), false)
    let none5_check = expect_bool_eq(none5_bool(false, false, false, false, false), true)
    let bool_ne_check = expect_bool_ne(true, false)
    let bool_not_check = expect_bool_not(false, true)
    let bool_and_check = expect_bool_and(true, false, false)
    let bool_or_check = expect_bool_or(false, true, true)
    let bool_xor_check = expect_bool_xor(true, true, false)
    let core_implies_check = expect_bool_eq(implies_bool(true, false), false)
    let core_ascending_check = expect_bool_eq(is_ascending_int(20, 21, 22), true)
    let core_ascending4_check = expect_bool_eq(is_ascending4_int(20, 21, 21, 22), true)
    let core_ascending5_check = expect_bool_eq(is_ascending5_int(20, 21, 21, 22, 23), true)
    let core_strict_ascending_check = expect_bool_eq(is_strictly_ascending_int(20, 20, 22), false)
    let core_strict_ascending4_check = expect_bool_eq(is_strictly_ascending4_int(20, 21, 22, 23), true)
    let core_strict_ascending5_check = expect_bool_eq(is_strictly_ascending5_int(20, 21, 22, 23, 24), true)
    let core_descending_check = expect_bool_eq(is_descending_int(22, 21, 20), true)
    let core_descending4_check = expect_bool_eq(is_descending4_int(22, 21, 21, 20), true)
    let core_descending5_check = expect_bool_eq(is_descending5_int(23, 22, 21, 21, 20), true)
    let core_strict_descending_check = expect_bool_eq(is_strictly_descending_int(22, 22, 20), false)
    let core_strict_descending4_check = expect_bool_eq(is_strictly_descending4_int(23, 22, 21, 20), true)
    let core_strict_descending5_check = expect_bool_eq(is_strictly_descending5_int(24, 23, 22, 21, 20), true)
    let core_bounds_check = expect_bool_eq(in_bounds_int(21, 22, 20), true)
    let core_exclusive_bounds_check = expect_bool_eq(in_exclusive_bounds_int(22, 22, 20), false)
    let core_within_check = expect_bool_eq(is_within_int(21, 22, 1), true)
    let core_not_within_check = expect_bool_eq(is_not_within_int(19, 22, 1), true)
    let core_outside_range_check = expect_bool_eq(is_outside_range_int(19, 20, 22), true)
    let core_outside_bounds_check = expect_bool_eq(is_outside_bounds_int(19, 22, 20), true)
    let range_check = expect_int_between(22, 20, 22)
    let exclusive_range_check = expect_int_exclusive_between(21, 20, 22)
    let outside_check = expect_int_outside(19, 20, 22)
    let bounds_check = expect_int_between_bounds(21, 22, 20)
    let exclusive_bounds_check = expect_int_exclusive_between_bounds(21, 22, 20)
    let outside_bounds_check = expect_int_outside_bounds(19, 22, 20)
    let clamp_min_expect_check = expect_int_clamp_min(19, 20, 20)
    let clamp_max_expect_check = expect_int_clamp_max(23, 22, 22)
    let clamped_check = expect_int_clamped(19, 20, 22, 20)
    let clamped_bounds_check = expect_int_clamped_bounds(23, 22, 20, 22)
    let distance_range_expect_check = expect_int_distance_to_range(19, 20, 22, 1)
    let distance_bounds_expect_check = expect_int_distance_to_bounds(23, 22, 20, 1)
    let bool_all3_check = expect_bool_all3(true, true, true, true)
    let bool_all4_check = expect_bool_all4(true, true, false, true, false)
    let bool_all5_check = expect_bool_all5(true, true, true, true, true, true)
    let bool_any3_check = expect_bool_any3(false, false, true, true)
    let bool_any4_check = expect_bool_any4(false, false, false, false, false)
    let bool_any5_check = expect_bool_any5(false, false, false, false, true, true)
    let bool_none3_check = expect_bool_none3(false, false, false, true)
    let bool_none4_check = expect_bool_none4(false, false, true, false, false)
    let bool_none5_check = expect_bool_none5(false, false, false, false, false, true)
    let bool_to_int_expect_check = expect_bool_to_int(true, 1)
    let array_sum_check = expect_int_array_sum3([2, 3, 4], 9)
    let array_sum5_check = expect_int_array_sum5([2, 3, 4, 5, 6], 20)
    let array_product_check = expect_int_array_product4([2, 3, 4, 5], 120)
    let array_extrema_check = expect_int_array_max5([3, 9, 5, 7, 11], 11) + expect_int_array_min5([3, 9, 5, 7, 1], 1)
    let array_bool_check = expect_bool_array_all3([true, true, true], true) + expect_bool_array_any5([false, false, false, false, true], true) + expect_bool_array_none4([false, false, false, false], true)
    let array_generic_check = expect_int_array_first3([8, 9, 10], 8) + expect_bool_array_last5([true, false, true, false, true], true)
    let max_expect_check = expect_int_max(20, 22, 22)
    let min_expect_check = expect_int_min(20, 22, 20)
    let max3_expect_check = expect_int_max3(20, 22, 21, 22)
    let min3_expect_check = expect_int_min3(20, 22, 21, 20)
    let max4_expect_check = expect_int_max4(20, 22, 21, 19, 22)
    let min4_expect_check = expect_int_min4(20, 22, 21, 19, 19)
    let max5_expect_check = expect_int_max5(20, 22, 21, 19, 23, 23)
    let min5_expect_check = expect_int_min5(20, 22, 21, 19, 18, 18)
    let median3_expect_check = expect_int_median3(22, 20, 21, 21)
    let sum3_expect_check = expect_int_sum3(2, 3, 4, 9)
    let sum4_expect_check = expect_int_sum4(2, 3, 4, 5, 14)
    let sum5_expect_check = expect_int_sum5(2, 3, 4, 5, 6, 20)
    let product3_expect_check = expect_int_product3(2, 3, 4, 24)
    let product4_expect_check = expect_int_product4(2, 3, 4, 5, 120)
    let product5_expect_check = expect_int_product5(2, 3, 4, 5, 6, 720)
    let average2_expect_check = expect_int_average2(5, 8, 6)
    let average3_expect_check = expect_int_average3(3, 6, 9, 6)
    let average4_expect_check = expect_int_average4(2, 4, 6, 8, 5)
    let average5_expect_check = expect_int_average5(2, 4, 6, 8, 10, 6)
    let sign_expect_check = expect_int_sign(0 - 5, 0 - 1)
    let sign_zero_expect_check = expect_int_sign(0, 0)
    let compare_less_expect_check = expect_int_compare(3, 9, 0 - 1)
    let compare_equal_expect_check = expect_int_compare(9, 9, 0)
    let compare_greater_expect_check = expect_int_compare(9, 3, 1)
    let abs_expect_check = expect_int_abs(0 - 22, 22)
    let abs_diff_expect_check = expect_int_abs_diff(22, 19, 3)
    let range_span_expect_check = expect_int_range_span(22, 20, 2)
    let lower_bound_expect_check = expect_int_lower_bound(22, 20, 20)
    let upper_bound_expect_check = expect_int_upper_bound(22, 20, 22)
    let quotient_expect_check = expect_int_quotient_or_zero(21, 7, 3)
    let quotient_zero_expect_check = expect_int_quotient_or_zero(21, 0, 0)
    let remainder_expect_check = expect_int_remainder_or_zero(22, 7, 1)
    let remainder_zero_expect_check = expect_int_remainder_or_zero(22, 0, 0)
    let has_remainder_expect_check = expect_int_has_remainder(22, 7)
    let factor_expect_check = expect_int_factor_of(7, 21)
    let ascending_check = expect_int_ascending(20, 21, 22)
    let ascending4_check = expect_int_ascending4(20, 21, 21, 22)
    let ascending5_check = expect_int_ascending5(20, 21, 21, 22, 23)
    let strict_ascending_check = expect_int_strictly_ascending(20, 21, 22)
    let strict_ascending4_check = expect_int_strictly_ascending4(20, 21, 22, 23)
    let strict_ascending5_check = expect_int_strictly_ascending5(20, 21, 22, 23, 24)
    let descending_check = expect_int_descending(22, 21, 20)
    let descending4_check = expect_int_descending4(22, 21, 21, 20)
    let descending5_check = expect_int_descending5(23, 22, 21, 21, 20)
    let strict_descending_check = expect_int_strictly_descending(22, 21, 20)
    let strict_descending4_check = expect_int_strictly_descending4(23, 22, 21, 20)
    let strict_descending5_check = expect_int_strictly_descending5(24, 23, 22, 21, 20)
    let divisible_check = expect_int_divisible_by(21, 7)
    let within_check = expect_int_within(21, 22, 1)
    let not_within_check = expect_int_not_within(19, 22, 1)
    let even_check = expect_int_even(22)
    let odd_check = expect_int_odd(21)
    let positive_check = expect_int_positive(22)
    let negative_check = expect_int_negative(0 - 1)
    let nonnegative_check = expect_int_nonnegative(0)
    let nonpositive_check = expect_int_nonpositive(0)
    let test_implies_check = expect_bool_implies(false, false)
    let true_check = expect_true(true)
    let status_ok_bool_check = expect_bool_eq(is_status_ok(0), true)
    let status_failed_bool_check = expect_bool_eq(is_status_failed(1), true)
    let merged_status_check = expect_int_eq(merge_status(0, 1), 1)
    let merged_status3_check = expect_int_eq(merge_status3(0, 1, 1), 2)
    let merged_status4_check = expect_int_eq(merge_status4(0, 1, 1, 1), 3)
    let merged_status5_check = expect_int_eq(merge_status5(0, 1, 1, 1, 1), 4)
    let merged_status6_check = expect_int_eq(merge_status6(0, 1, 1, 1, 1, 1), 5)
    let status_ok_check = expect_status_ok(merge_status(0, 0))
    let status_failed_check = expect_status_failed(merge_status(0, 1))
    let failed_status_ok_check = expect_int_eq(expect_status_ok(1), 1)
    let failed_status_failed_check = expect_int_eq(expect_status_failed(0), 1)
    let failed_bool_ne_check = expect_int_eq(expect_bool_ne(true, true), 1)
    let failed_bool_not_check = expect_int_eq(expect_bool_not(false, false), 1)
    let failed_bool_and_check = expect_int_eq(expect_bool_and(true, false, true), 1)
    let failed_bool_or_check = expect_int_eq(expect_bool_or(false, false, true), 1)
    let failed_bool_xor_check = expect_int_eq(expect_bool_xor(true, false, false), 1)
    let failed_bool_all3_check = expect_int_eq(expect_bool_all3(true, true, false, true), 1)
    let failed_bool_all4_check = expect_int_eq(expect_bool_all4(true, true, true, true, false), 1)
    let failed_bool_all5_check = expect_int_eq(expect_bool_all5(true, true, true, true, false, true), 1)
    let failed_bool_any3_check = expect_int_eq(expect_bool_any3(false, false, false, true), 1)
    let failed_bool_any4_check = expect_int_eq(expect_bool_any4(false, false, true, false, false), 1)
    let failed_bool_any5_check = expect_int_eq(expect_bool_any5(false, false, false, false, false, true), 1)
    let failed_bool_none3_check = expect_int_eq(expect_bool_none3(false, true, false, true), 1)
    let failed_bool_none4_check = expect_int_eq(expect_bool_none4(false, false, false, false, false), 1)
    let failed_bool_none5_check = expect_int_eq(expect_bool_none5(false, false, true, false, false, true), 1)
    let failed_bool_to_int_check = expect_int_eq(expect_bool_to_int(false, 1), 1)
    let failed_range_check = expect_int_eq(expect_int_between(19, 20, 22), 1)
    let failed_exclusive_range_check = expect_int_eq(expect_int_exclusive_between(20, 20, 22), 1)
    let failed_outside_check = expect_int_eq(expect_int_outside(21, 20, 22), 1)
    let failed_bounds_check = expect_int_eq(expect_int_between_bounds(19, 22, 20), 1)
    let failed_exclusive_bounds_check = expect_int_eq(expect_int_exclusive_between_bounds(22, 22, 20), 1)
    let failed_outside_bounds_check = expect_int_eq(expect_int_outside_bounds(21, 22, 20), 1)
    let failed_clamp_min_check = expect_int_eq(expect_int_clamp_min(19, 20, 19), 1)
    let failed_clamp_max_check = expect_int_eq(expect_int_clamp_max(23, 22, 23), 1)
    let failed_clamped_check = expect_int_eq(expect_int_clamped(19, 20, 22, 19), 1)
    let failed_clamped_bounds_check = expect_int_eq(expect_int_clamped_bounds(23, 22, 20, 23), 1)
    let failed_distance_range_check = expect_int_eq(expect_int_distance_to_range(21, 20, 22, 1), 1)
    let failed_distance_bounds_check = expect_int_eq(expect_int_distance_to_bounds(21, 22, 20, 1), 1)
    let failed_max_check = expect_int_eq(expect_int_max(20, 22, 20), 1)
    let failed_min_check = expect_int_eq(expect_int_min(20, 22, 22), 1)
    let failed_max3_check = expect_int_eq(expect_int_max3(20, 22, 21, 21), 1)
    let failed_min3_check = expect_int_eq(expect_int_min3(20, 22, 21, 21), 1)
    let failed_max4_check = expect_int_eq(expect_int_max4(20, 22, 21, 19, 21), 1)
    let failed_min4_check = expect_int_eq(expect_int_min4(20, 22, 21, 19, 20), 1)
    let failed_max5_check = expect_int_eq(expect_int_max5(20, 22, 21, 19, 23, 22), 1)
    let failed_min5_check = expect_int_eq(expect_int_min5(20, 22, 21, 19, 18, 19), 1)
    let failed_median3_check = expect_int_eq(expect_int_median3(22, 20, 21, 22), 1)
    let failed_sum3_check = expect_int_eq(expect_int_sum3(2, 3, 4, 10), 1)
    let failed_sum4_check = expect_int_eq(expect_int_sum4(2, 3, 4, 5, 15), 1)
    let failed_sum5_check = expect_int_eq(expect_int_sum5(2, 3, 4, 5, 6, 21), 1)
    let failed_product3_check = expect_int_eq(expect_int_product3(2, 3, 4, 25), 1)
    let failed_product4_check = expect_int_eq(expect_int_product4(2, 3, 4, 5, 121), 1)
    let failed_product5_check = expect_int_eq(expect_int_product5(2, 3, 4, 5, 6, 721), 1)
    let failed_average2_check = expect_int_eq(expect_int_average2(5, 8, 7), 1)
    let failed_average3_check = expect_int_eq(expect_int_average3(3, 6, 9, 7), 1)
    let failed_average4_check = expect_int_eq(expect_int_average4(2, 4, 6, 8, 6), 1)
    let failed_average5_check = expect_int_eq(expect_int_average5(2, 4, 6, 8, 10, 7), 1)
    let failed_sign_check = expect_int_eq(expect_int_sign(5, 0 - 1), 1)
    let failed_compare_equal_check = expect_int_eq(expect_int_compare(9, 9, 1), 1)
    let failed_compare_order_check = expect_int_eq(expect_int_compare(3, 9, 1), 1)
    let failed_abs_check = expect_int_eq(expect_int_abs(0 - 22, 0 - 22), 1)
    let failed_abs_diff_check = expect_int_eq(expect_int_abs_diff(22, 19, 2), 1)
    let failed_range_span_check = expect_int_eq(expect_int_range_span(22, 20, 3), 1)
    let failed_lower_bound_check = expect_int_eq(expect_int_lower_bound(22, 20, 22), 1)
    let failed_upper_bound_check = expect_int_eq(expect_int_upper_bound(22, 20, 20), 1)
    let failed_quotient_check = expect_int_eq(expect_int_quotient_or_zero(21, 7, 4), 1)
    let failed_quotient_zero_check = expect_int_eq(expect_int_quotient_or_zero(21, 0, 1), 1)
    let failed_remainder_check = expect_int_eq(expect_int_remainder_or_zero(22, 7, 2), 1)
    let failed_remainder_zero_check = expect_int_eq(expect_int_remainder_or_zero(22, 0, 1), 1)
    let failed_has_remainder_check = expect_int_eq(expect_int_has_remainder(21, 7), 1)
    let failed_factor_check = expect_int_eq(expect_int_factor_of(0, 21), 1)
    let failed_ascending_check = expect_int_eq(expect_int_ascending(22, 21, 20), 1)
    let failed_ascending4_check = expect_int_eq(expect_int_ascending4(20, 22, 21, 23), 1)
    let failed_ascending5_check = expect_int_eq(expect_int_ascending5(20, 21, 23, 22, 24), 1)
    let failed_strict_ascending_check = expect_int_eq(expect_int_strictly_ascending(20, 20, 22), 1)
    let failed_strict_ascending4_check = expect_int_eq(expect_int_strictly_ascending4(20, 21, 21, 23), 1)
    let failed_strict_ascending5_check = expect_int_eq(expect_int_strictly_ascending5(20, 21, 22, 23, 23), 1)
    let failed_descending_check = expect_int_eq(expect_int_descending(20, 22, 21), 1)
    let failed_descending4_check = expect_int_eq(expect_int_descending4(22, 20, 21, 19), 1)
    let failed_descending5_check = expect_int_eq(expect_int_descending5(23, 22, 20, 21, 19), 1)
    let failed_strict_descending_check = expect_int_eq(expect_int_strictly_descending(22, 22, 20), 1)
    let failed_strict_descending4_check = expect_int_eq(expect_int_strictly_descending4(23, 22, 22, 20), 1)
    let failed_strict_descending5_check = expect_int_eq(expect_int_strictly_descending5(24, 23, 22, 22, 20), 1)
    let failed_divisible_check = expect_int_eq(expect_int_divisible_by(21, 0), 1)
    let failed_within_check = expect_int_eq(expect_int_within(19, 22, 1), 1)
    let failed_not_within_check = expect_int_eq(expect_int_not_within(22, 22, 0), 1)
    let failed_even_check = expect_int_eq(expect_int_even(21), 1)
    let failed_odd_check = expect_int_eq(expect_int_odd(22), 1)
    let failed_positive_check = expect_int_eq(expect_int_positive(0), 1)
    let failed_negative_check = expect_int_eq(expect_int_negative(0), 1)
    let failed_nonnegative_check = expect_int_eq(expect_int_nonnegative(0 - 1), 1)
    let failed_nonpositive_check = expect_int_eq(expect_int_nonpositive(1), 1)
    let failed_implies_check = expect_int_eq(expect_bool_implies(true, false), 1)
    let option_status = merge_status4(expect_int_option_some(some_int(22), 22), expect_int_option_none(none_int()), expect_bool_option_some(some_bool(true), true), expect_bool_option_none(none_bool()))
    let option_or_status = merge_status4(expect_int_option_or(none_int(), some_int(9), 9), expect_bool_option_or(none_bool(), some_bool(false), false), 0, 0)
    let result_status = merge_status4(expect_int_result_ok(result_ok_int(22), 22), expect_int_result_err(result_err_int(7), 7), expect_bool_result_ok(result_ok_bool(true), true), expect_bool_result_err(result_err_bool(3), 3))
    let result_or_status = merge_status4(expect_int_result_or(result_err_int(5), result_ok_int(9), 9), expect_bool_result_or(result_err_bool(6), result_ok_bool(false), false), 0, 0)
    let conversion_status = merge_status4(expect_int_result_to_option_some(result_ok_int(31), 31), expect_int_result_to_option_none(result_err_int(8)), expect_int_option_ok_or(some_int(31), 8, 31), expect_int_option_ok_or_err(none_int(), 8))
    let conversion_bool_status = merge_status4(expect_bool_result_to_option_some(result_ok_bool(false), false), expect_bool_result_to_option_none(result_err_bool(9)), expect_bool_option_ok_or(some_bool(true), 9, true), expect_bool_option_ok_or_err(none_bool(), 9))
    let error_option_status = merge_status4(expect_int_result_error_some(result_err_int(0), 0), expect_int_result_error_none(result_ok_int(31)), expect_bool_result_error_some(result_err_bool(0), 0), expect_bool_result_error_none(result_ok_bool(false)))
    let generic_none_int: Option[Int] = option_none()
    let generic_option_status = merge_status4(expect_generic_int_option_some(Option.Some(7), 7), expect_generic_int_option_none(generic_none_int), expect_generic_bool_option_some(Option.Some(true), true), expect_generic_bool_option_none(Option.None))
    let generic_option_or_status = merge_status4(expect_generic_int_option_or(generic_none_int, Option.Some(9), 9), expect_generic_bool_option_or(Option.None, Option.Some(false), false), 0, 0)
    let generic_result_status = merge_status4(expect_generic_int_result_ok(Result.Ok(7), 7), expect_generic_int_result_err(Result.Err(3), 3), expect_generic_bool_result_ok(Result.Ok(true), true), expect_generic_bool_result_err(Result.Err(4), 4))
    let generic_result_or_status = merge_status4(expect_generic_int_result_or(Result.Err(5), Result.Ok(11), 11), expect_generic_bool_result_or(Result.Err(6), Result.Ok(false), false), 0, 0)
    let generic_result_error_status = merge_status4(expect_generic_int_result_error(Result.Err(8), 0, 8), expect_generic_int_result_error(Result.Ok(14), 0, 0), expect_generic_bool_result_error(Result.Err(9), 0, 9), expect_generic_bool_result_error(Result.Ok(false), 0, 0))
    let generic_none_bool: Option[Bool] = option_none()
    let generic_result_conversion_status = merge_status4(expect_generic_int_result_to_option_some(Result.Ok(31), 31), expect_generic_int_result_to_option_none(Result.Err(8)), expect_generic_bool_result_to_option_some(Result.Ok(false), false), expect_generic_bool_result_to_option_none(Result.Err(9)))
    let generic_result_error_option_status = merge_status4(expect_generic_int_result_error_some(Result.Err(0), 0), expect_generic_int_result_error_none(Result.Ok(31)), expect_generic_bool_result_error_some(Result.Err(0), 0), expect_generic_bool_result_error_none(Result.Ok(false)))
    let generic_option_conversion_status = merge_status4(expect_generic_int_option_ok_or(Option.Some(31), 8, 31), expect_generic_int_option_ok_or_err(generic_none_int, 8), expect_generic_bool_option_ok_or(Option.Some(true), 9, true), expect_generic_bool_option_ok_or_err(generic_none_bool, 9))

    let core_status = merge_status6(max_check + max3_check + max4_check + max5_check + min_check, min3_check + min4_check + min5_check + median3_check + sum3_check, sum4_check + sum5_check + product3_check + product4_check, product5_check + average2_check + average3_check + average4_check + average5_check + quotient_check, quotient_zero_check + remainder_check + remainder_zero_check + has_remainder_check + factor_check + clamp_min_check, clamp_max_check + clamp_bounds_check + abs_check + abs_diff_check + range_span_check + compare_check + sign_negative_check + sign_zero_check + sign_positive_check + and_check + xor_check + all3_check + all4_check + all5_check + any3_check + any4_check + any5_check + none3_check + none4_check + none5_check + bool_ne_check + bool_not_check + bool_and_check + bool_or_check + core_descending_check + core_descending4_check + core_descending5_check + core_strict_descending_check + core_strict_descending4_check + core_strict_descending5_check + core_not_within_check + core_outside_range_check + core_outside_bounds_check + lower_bound_check + upper_bound_check + distance_range_check + distance_bounds_check)
    let bool_status = merge_status4(bool_xor_check + core_implies_check + core_ascending_check + core_ascending4_check + core_ascending5_check + core_strict_ascending_check + core_strict_ascending4_check + core_strict_ascending5_check, core_bounds_check + core_exclusive_bounds_check + core_within_check + range_check, bool_all3_check + bool_all4_check + bool_all5_check + bool_any3_check + bool_any4_check + bool_any5_check, bool_none3_check + bool_none4_check + bool_none5_check + bool_to_int_expect_check + failed_bool_ne_check + failed_bool_not_check + failed_bool_and_check + failed_bool_or_check + failed_bool_xor_check + failed_bool_all3_check + failed_bool_all4_check + failed_bool_all5_check + failed_bool_any3_check + failed_bool_any4_check + failed_bool_any5_check + failed_bool_none3_check + failed_bool_none4_check + failed_bool_none5_check + failed_bool_to_int_check + exclusive_range_check + outside_check + bounds_check)
    let range_status = merge_status5(exclusive_bounds_check + outside_bounds_check + clamp_min_expect_check + clamp_max_expect_check + clamped_check + clamped_bounds_check, distance_range_expect_check + distance_bounds_expect_check + max_expect_check + min_expect_check, max3_expect_check + min3_expect_check + max4_expect_check + min4_expect_check + max5_expect_check + min5_expect_check + median3_expect_check, sum3_expect_check + sum4_expect_check + sum5_expect_check + product3_expect_check + product4_expect_check + product5_expect_check + average2_expect_check + average3_expect_check + average4_expect_check + average5_expect_check, sign_expect_check + sign_zero_expect_check + compare_less_expect_check + compare_equal_expect_check + compare_greater_expect_check + abs_expect_check + abs_diff_expect_check + range_span_expect_check + lower_bound_expect_check + upper_bound_expect_check + quotient_expect_check + quotient_zero_expect_check + remainder_expect_check + remainder_zero_expect_check + has_remainder_expect_check + factor_expect_check + ascending_check + ascending4_check + ascending5_check + strict_ascending_check + strict_ascending4_check + strict_ascending5_check + descending_check + descending4_check + descending5_check + strict_descending_check + strict_descending4_check + strict_descending5_check + divisible_check + within_check + not_within_check + even_check + odd_check + positive_check + negative_check + nonnegative_check + nonpositive_check + test_implies_check + true_check + status_ok_bool_check)
    let status_helper_status = merge_status4(status_failed_bool_check + merged_status_check + merged_status3_check + merged_status4_check, merged_status5_check + merged_status6_check + status_ok_check + status_failed_check, failed_status_ok_check + failed_status_failed_check + failed_range_check + failed_exclusive_range_check, failed_outside_check + failed_bounds_check + failed_exclusive_bounds_check + failed_outside_bounds_check)
    let failure_status = merge_status4(failed_clamp_min_check + failed_clamp_max_check + failed_clamped_check + failed_clamped_bounds_check + failed_distance_range_check + failed_distance_bounds_check, failed_max_check + failed_min_check + failed_max3_check + failed_min3_check + failed_max4_check + failed_min4_check + failed_max5_check + failed_min5_check + failed_median3_check, failed_sum3_check + failed_sum4_check + failed_sum5_check + failed_product3_check + failed_product4_check + failed_product5_check + failed_average2_check + failed_average3_check + failed_average4_check + failed_average5_check + failed_sign_check + failed_compare_equal_check + failed_compare_order_check + failed_abs_check + failed_abs_diff_check + failed_range_span_check + failed_lower_bound_check + failed_upper_bound_check + failed_quotient_check + failed_quotient_zero_check, failed_remainder_check + failed_remainder_zero_check + failed_has_remainder_check + failed_factor_check + failed_ascending_check + failed_ascending4_check + failed_ascending5_check + failed_strict_ascending_check + failed_strict_ascending4_check + failed_strict_ascending5_check + failed_descending_check + failed_descending4_check + failed_descending5_check + failed_strict_descending_check + failed_strict_descending4_check + failed_strict_descending5_check + failed_divisible_check + failed_within_check + failed_not_within_check + failed_even_check + failed_odd_check + failed_positive_check + failed_negative_check + failed_nonnegative_check + failed_nonpositive_check + failed_implies_check)
    let array_status = merge_status6(array_sum_check, array_sum5_check, array_product_check, array_extrema_check, array_bool_check, array_generic_check)

    return expect_status_ok(merge_status6(core_status, bool_status, range_status, status_helper_status, failure_status, merge_status6(array_status + option_status, option_or_status, result_status, result_or_status, conversion_status + generic_option_status + generic_option_or_status + generic_result_status + generic_result_conversion_status, conversion_bool_status + error_option_status + generic_result_or_status + generic_result_error_status + generic_result_error_option_status + generic_option_conversion_status)))
}
"#
}

#[test]
fn project_init_creates_package_scaffold_and_check_succeeds() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-init-package");
    let project_root = temp.path().join("demo-package");

    let mut init = ql_command(&workspace_root);
    init.args(["project", "init", &project_root.to_string_lossy()]);
    let output = run_command_capture(&mut init, "`ql project init` package");
    let (stdout, stderr) = expect_success("project-init-package", "package init", &output).unwrap();
    expect_empty_stderr("project-init-package", "package init", &stderr).unwrap();
    expect_stdout_contains_all(
        "project-init-package",
        &stdout,
        &[
            &format!(
                "created: {}",
                project_root
                    .join("qlang.toml")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            &format!(
                "created: {}",
                project_root
                    .join("src")
                    .join("lib.ql")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            &format!(
                "created: {}",
                project_root
                    .join("src")
                    .join("main.ql")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            &format!(
                "created: {}",
                project_root
                    .join("tests")
                    .join("smoke.ql")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
        ],
    )
    .unwrap();

    assert_eq!(
        read_normalized_file(&project_root.join("qlang.toml"), "package manifest"),
        "[package]\nname = \"demo-package\"\n"
    );
    assert_eq!(
        read_normalized_file(&project_root.join("src/lib.ql"), "package source"),
        "pub fn run() -> Int {\n    return 0\n}\n"
    );
    assert_eq!(
        read_normalized_file(&project_root.join("src/main.ql"), "package main source"),
        "fn main() -> Int {\n    return 0\n}\n"
    );
    assert_eq!(
        read_normalized_file(&project_root.join("tests/smoke.ql"), "package smoke test"),
        "fn main() -> Int {\n    return 0\n}\n"
    );

    let mut check = ql_command(&workspace_root);
    check.args(["check", &project_root.to_string_lossy()]);
    let output = run_command_capture(&mut check, "`ql check` initialized package");
    let (stdout, stderr) =
        expect_success("project-init-package", "check initialized package", &output).unwrap();
    expect_empty_stderr("project-init-package", "check initialized package", &stderr).unwrap();
    expect_stdout_contains_all(
        "project-init-package",
        &stdout,
        &[&format!(
            "ok: {}",
            project_root.join("src").join("lib.ql").to_string_lossy()
        )],
    )
    .unwrap();
}

#[test]
fn project_init_with_stdlib_creates_consuming_package_scaffold_and_check_succeeds() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-init-stdlib-package");
    let stdlib_root = write_repo_stdlib_fixture(&temp, &workspace_root);
    let project_root = temp.path().join("demo-package");

    let mut init = ql_command(&workspace_root);
    init.args([
        "project",
        "init",
        &project_root.to_string_lossy(),
        "--stdlib",
        &stdlib_root.to_string_lossy(),
    ]);
    let output = run_command_capture(&mut init, "`ql project init --stdlib` package");
    let (_stdout, stderr) = expect_success(
        "project-init-stdlib-package",
        "stdlib package init",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-package",
        "stdlib package init",
        &stderr,
    )
    .unwrap();

    assert_eq!(
        read_normalized_file(&project_root.join("qlang.toml"), "stdlib package manifest"),
        "[package]\nname = \"demo-package\"\n\n[dependencies]\n\"std.core\" = \"../stdlib/packages/core\"\n\"std.option\" = \"../stdlib/packages/option\"\n\"std.result\" = \"../stdlib/packages/result\"\n\"std.array\" = \"../stdlib/packages/array\"\n\"std.test\" = \"../stdlib/packages/test\"\n"
    );
    assert_eq!(
        read_normalized_file(&project_root.join("src/lib.ql"), "stdlib package source"),
        "use std.array.first3_array as first3_array\nuse std.array.sum3_int_array as sum3_int_array\nuse std.core.clamp_int as clamp_int\nuse std.option.some as option_some\nuse std.option.unwrap_or as option_unwrap_or\nuse std.result.Result as Result\nuse std.result.ok as result_ok\nuse std.result.unwrap_result_or as result_unwrap_result_or\n\npub fn run() -> Int {\n    let result_value: Result[Int, Int] = result_ok(option_unwrap_or(option_some(42), 0))\n    return clamp_int(result_unwrap_result_or(result_value, 0) + sum3_int_array([1, 2, first3_array([3, 4, 5])]), 0, 100)\n}\n"
    );
    assert_eq!(
        read_normalized_file(
            &project_root.join("tests/smoke.ql"),
            "stdlib package smoke test"
        ),
        expected_stdlib_package_smoke_source()
    );

    let mut check = ql_command(&workspace_root);
    check.args(["check", &project_root.to_string_lossy()]);
    let output = run_command_capture(&mut check, "`ql check` initialized stdlib package");
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-package",
        "check initialized stdlib package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-package",
        "check initialized stdlib package",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "project-init-stdlib-package",
        &stdout.replace('\\', "/"),
        &[
            &format!(
                "ok: {}",
                project_root
                    .join("src")
                    .join("lib.ql")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            "loaded interface:",
        ],
    )
    .unwrap();
}

#[test]
fn project_init_creates_runnable_package_scaffold() {
    if !toolchain_available("`ql project init` runnable package test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-init-package-run");
    let project_root = temp.path().join("demo-package");
    let output_path = executable_output_path(&project_root.join("target/ql/debug"), "main");

    let mut init = ql_command(&workspace_root);
    init.args(["project", "init", &project_root.to_string_lossy()]);
    let output = run_command_capture(&mut init, "`ql project init` runnable package");
    let (_stdout, stderr) = expect_success(
        "project-init-package-run",
        "package init for runnable scaffold",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-package-run",
        "package init for runnable scaffold",
        &stderr,
    )
    .unwrap();

    let mut run = ql_command(&workspace_root);
    run.current_dir(temp.path());
    run.args(["run"]).arg(&project_root);
    let output = run_command_capture(&mut run, "`ql run` initialized package");
    let (stdout, stderr) = expect_exit_code(
        "project-init-package-run",
        "run initialized package",
        &output,
        0,
    )
    .unwrap();
    expect_silent_output(
        "project-init-package-run",
        "run initialized package",
        &stdout,
        &stderr,
    )
    .unwrap();
    expect_file_exists(
        "project-init-package-run",
        &output_path,
        "initialized package executable",
        "run initialized package",
    )
    .unwrap();
}

#[test]
fn project_init_with_stdlib_creates_runnable_and_testable_package_scaffold() {
    if !toolchain_available("`ql project init --stdlib` runnable package test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-init-stdlib-package-run");
    let stdlib_root = write_repo_stdlib_fixture(&temp, &workspace_root);
    let project_root = temp.path().join("demo-package");

    let mut init = ql_command(&workspace_root);
    init.args([
        "project",
        "init",
        &project_root.to_string_lossy(),
        "--stdlib",
        &stdlib_root.to_string_lossy(),
    ]);
    let output = run_command_capture(&mut init, "`ql project init --stdlib` runnable package");
    let (_stdout, stderr) = expect_success(
        "project-init-stdlib-package-run",
        "stdlib package init for runnable scaffold",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-package-run",
        "stdlib package init for runnable scaffold",
        &stderr,
    )
    .unwrap();

    let mut run = ql_command(&workspace_root);
    run.current_dir(temp.path());
    run.args(["run"]).arg(&project_root);
    let output = run_command_capture(&mut run, "`ql run` initialized stdlib package");
    let (stdout, stderr) = expect_exit_code(
        "project-init-stdlib-package-run",
        "run initialized stdlib package",
        &output,
        0,
    )
    .unwrap();
    expect_silent_output(
        "project-init-stdlib-package-run",
        "run initialized stdlib package",
        &stdout,
        &stderr,
    )
    .unwrap();

    let mut test = ql_command(&workspace_root);
    test.current_dir(temp.path());
    test.args(["test"]).arg(&project_root);
    let output = run_command_capture(&mut test, "`ql test` initialized stdlib package");
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-package-run",
        "test initialized stdlib package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-package-run",
        "test initialized stdlib package",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "project-init-stdlib-package-run",
        &stdout.replace('\\', "/"),
        &[
            "test tests/smoke.ql ... ok",
            "test result: ok. 1 passed; 0 failed",
        ],
    )
    .unwrap();
}

#[test]
fn project_init_creates_workspace_scaffold_and_graph_succeeds() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-init-workspace");
    let project_root = temp.path().join("demo-workspace");

    let mut init = ql_command(&workspace_root);
    init.args([
        "project",
        "init",
        &project_root.to_string_lossy(),
        "--workspace",
        "--name",
        "app",
    ]);
    let output = run_command_capture(&mut init, "`ql project init --workspace`");
    let (stdout, stderr) =
        expect_success("project-init-workspace", "workspace init", &output).unwrap();
    expect_empty_stderr("project-init-workspace", "workspace init", &stderr).unwrap();
    expect_stdout_contains_all(
        "project-init-workspace",
        &stdout,
        &[
            &format!(
                "created: {}",
                project_root
                    .join("qlang.toml")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            &format!(
                "created: {}",
                project_root
                    .join("packages")
                    .join("app")
                    .join("qlang.toml")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            &format!(
                "created: {}",
                project_root
                    .join("packages")
                    .join("app")
                    .join("src")
                    .join("main.ql")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            &format!(
                "created: {}",
                project_root
                    .join("packages")
                    .join("app")
                    .join("src")
                    .join("lib.ql")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            &format!(
                "created: {}",
                project_root
                    .join("packages")
                    .join("app")
                    .join("tests")
                    .join("smoke.ql")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
        ],
    )
    .unwrap();

    assert_eq!(
        read_normalized_file(&project_root.join("qlang.toml"), "workspace manifest"),
        "[workspace]\nmembers = [\"packages/app\"]\n"
    );
    assert_eq!(
        read_normalized_file(
            &project_root.join("packages/app/qlang.toml"),
            "workspace member manifest"
        ),
        "[package]\nname = \"app\"\n"
    );
    assert_eq!(
        read_normalized_file(
            &project_root.join("packages/app/src/main.ql"),
            "workspace member main source"
        ),
        "fn main() -> Int {\n    return 0\n}\n"
    );
    assert_eq!(
        read_normalized_file(
            &project_root.join("packages/app/tests/smoke.ql"),
            "workspace member smoke test"
        ),
        "fn main() -> Int {\n    return 0\n}\n"
    );

    let mut graph = ql_command(&workspace_root);
    graph.args(["project", "graph", &project_root.to_string_lossy()]);
    let output = run_command_capture(&mut graph, "`ql project graph` initialized workspace");
    let (stdout, stderr) = expect_success(
        "project-init-workspace",
        "graph initialized workspace",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-workspace",
        "graph initialized workspace",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "project-init-workspace",
        &stdout,
        &[
            "package: <none>",
            "workspace_members:",
            "  - packages/app",
            "workspace_packages:",
            "  - member: packages/app",
            "    package: app",
            "    status: missing",
        ],
    )
    .unwrap();
}

#[test]
fn project_init_with_stdlib_creates_consuming_workspace_scaffold_and_check_succeeds() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-init-stdlib-workspace");
    let stdlib_root = write_repo_stdlib_fixture(&temp, &workspace_root);
    let project_root = temp.path().join("demo-workspace");
    let member_root = project_root.join("packages/app");

    let mut init = ql_command(&workspace_root);
    init.args([
        "project",
        "init",
        &project_root.to_string_lossy(),
        "--workspace",
        "--name",
        "app",
        "--stdlib",
        &stdlib_root.to_string_lossy(),
    ]);
    let output = run_command_capture(&mut init, "`ql project init --workspace --stdlib`");
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-workspace",
        "stdlib workspace init",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-workspace",
        "stdlib workspace init",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "project-init-stdlib-workspace",
        &stdout,
        &[
            &format!(
                "created: {}",
                project_root
                    .join("qlang.toml")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            &format!(
                "created: {}",
                member_root
                    .join("qlang.toml")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            &format!(
                "created: {}",
                member_root
                    .join("tests/smoke.ql")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
        ],
    )
    .unwrap();

    assert_eq!(
        read_normalized_file(
            &project_root.join("qlang.toml"),
            "stdlib workspace manifest"
        ),
        "[workspace]\nmembers = [\"packages/app\"]\n"
    );
    assert_eq!(
        read_normalized_file(
            &member_root.join("qlang.toml"),
            "stdlib workspace member manifest"
        ),
        "[package]\nname = \"app\"\n\n[dependencies]\n\"std.core\" = \"../../../stdlib/packages/core\"\n\"std.option\" = \"../../../stdlib/packages/option\"\n\"std.result\" = \"../../../stdlib/packages/result\"\n\"std.array\" = \"../../../stdlib/packages/array\"\n\"std.test\" = \"../../../stdlib/packages/test\"\n"
    );
    assert_eq!(
        read_normalized_file(
            &member_root.join("src/lib.ql"),
            "stdlib workspace member source"
        ),
        "use std.array.first3_array as first3_array\nuse std.array.sum3_int_array as sum3_int_array\nuse std.core.clamp_int as clamp_int\nuse std.option.some as option_some\nuse std.option.unwrap_or as option_unwrap_or\nuse std.result.Result as Result\nuse std.result.ok as result_ok\nuse std.result.unwrap_result_or as result_unwrap_result_or\n\npub fn run() -> Int {\n    let result_value: Result[Int, Int] = result_ok(option_unwrap_or(option_some(42), 0))\n    return clamp_int(result_unwrap_result_or(result_value, 0) + sum3_int_array([1, 2, first3_array([3, 4, 5])]), 0, 100)\n}\n"
    );
    assert_eq!(
        read_normalized_file(
            &member_root.join("tests/smoke.ql"),
            "stdlib workspace member smoke test"
        ),
        expected_stdlib_package_smoke_source()
    );

    let mut check = ql_command(&workspace_root);
    check.args(["check", &project_root.to_string_lossy()]);
    let output = run_command_capture(&mut check, "`ql check` initialized stdlib workspace");
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-workspace",
        "check initialized stdlib workspace",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-workspace",
        "check initialized stdlib workspace",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "project-init-stdlib-workspace",
        &stdout.replace('\\', "/"),
        &[
            &format!(
                "ok: {}",
                member_root
                    .join("src/lib.ql")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            "loaded interface:",
        ],
    )
    .unwrap();
}

#[test]
fn project_init_with_stdlib_creates_runnable_and_testable_workspace_scaffold() {
    if !toolchain_available("`ql project init --workspace --stdlib` runnable workspace test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-init-stdlib-workspace-run");
    let stdlib_root = write_repo_stdlib_fixture(&temp, &workspace_root);
    let project_root = temp.path().join("demo-workspace");

    let mut init = ql_command(&workspace_root);
    init.args([
        "project",
        "init",
        &project_root.to_string_lossy(),
        "--workspace",
        "--name",
        "app",
        "--stdlib",
        &stdlib_root.to_string_lossy(),
    ]);
    let output = run_command_capture(
        &mut init,
        "`ql project init --workspace --stdlib` runnable workspace",
    );
    let (_stdout, stderr) = expect_success(
        "project-init-stdlib-workspace-run",
        "stdlib workspace init for runnable scaffold",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-workspace-run",
        "stdlib workspace init for runnable scaffold",
        &stderr,
    )
    .unwrap();

    let mut run = ql_command(&workspace_root);
    run.current_dir(temp.path());
    run.args(["run"]).arg(&project_root);
    let output = run_command_capture(&mut run, "`ql run` initialized stdlib workspace");
    let (stdout, stderr) = expect_exit_code(
        "project-init-stdlib-workspace-run",
        "run initialized stdlib workspace",
        &output,
        0,
    )
    .unwrap();
    expect_silent_output(
        "project-init-stdlib-workspace-run",
        "run initialized stdlib workspace",
        &stdout,
        &stderr,
    )
    .unwrap();

    let mut test = ql_command(&workspace_root);
    test.current_dir(temp.path());
    test.args(["test"]).arg(&project_root);
    let output = run_command_capture(&mut test, "`ql test` initialized stdlib workspace");
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-workspace-run",
        "test initialized stdlib workspace",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-workspace-run",
        "test initialized stdlib workspace",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "project-init-stdlib-workspace-run",
        &stdout.replace('\\', "/"),
        &[
            "test packages/app/tests/smoke.ql ... ok",
            "test result: ok. 1 passed; 0 failed",
        ],
    )
    .unwrap();
}

#[test]
fn project_init_refuses_to_overwrite_existing_manifest() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-init-conflict");
    let project_root = temp.path().join("demo-conflict");
    temp.write(
        "demo-conflict/qlang.toml",
        "[package]\nname = \"already-there\"\n",
    );

    let mut init = ql_command(&workspace_root);
    init.args(["project", "init", &project_root.to_string_lossy()]);
    let output = run_command_capture(&mut init, "`ql project init` conflicting manifest");
    let (stdout, stderr) = support::expect_exit_code(
        "project-init-conflict",
        "conflicting package init",
        &output,
        1,
    )
    .unwrap();
    expect_empty_stdout("project-init-conflict", "conflicting package init", &stdout).unwrap();
    expect_stderr_contains(
        "project-init-conflict",
        "conflicting package init",
        &stderr,
        &format!(
            "error: `ql project init` would overwrite existing path `{}`",
            project_root
                .join("qlang.toml")
                .to_string_lossy()
                .replace('\\', "/")
        ),
    )
    .unwrap();
}

#[test]
fn project_add_creates_workspace_member_from_member_source_path() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-add-success");
    let project_root = temp.path().join("workspace");
    let request_path = project_root.join("packages/app/src/main.ql");

    let mut init = ql_command(&workspace_root);
    init.args([
        "project",
        "init",
        &project_root.to_string_lossy(),
        "--workspace",
        "--name",
        "app",
    ]);
    let output = run_command_capture(&mut init, "`ql project init` workspace for add");
    let (_stdout, stderr) =
        expect_success("project-add-success", "workspace init for add", &output).unwrap();
    expect_empty_stderr("project-add-success", "workspace init for add", &stderr).unwrap();

    let mut add_core = ql_command(&workspace_root);
    add_core.args([
        "project",
        "add",
        &project_root.to_string_lossy(),
        "--name",
        "core",
    ]);
    let output = run_command_capture(&mut add_core, "`ql project add` workspace core member");
    let (_stdout, stderr) =
        expect_success("project-add-success", "add workspace core member", &output).unwrap();
    expect_empty_stderr("project-add-success", "add workspace core member", &stderr).unwrap();

    let mut add = ql_command(&workspace_root);
    add.args([
        "project",
        "add",
        &request_path.to_string_lossy(),
        "--name",
        "tools",
        "--dependency",
        "app",
        "--dependency",
        "core",
    ]);
    let output = run_command_capture(&mut add, "`ql project add` workspace member source path");
    let (stdout, stderr) =
        expect_success("project-add-success", "add workspace member", &output).unwrap();
    expect_empty_stderr("project-add-success", "add workspace member", &stderr).unwrap();
    expect_stdout_contains_all(
        "project-add-success",
        &stdout,
        &[
            &format!(
                "updated: {}",
                project_root
                    .join("qlang.toml")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            &format!(
                "created: {}",
                project_root
                    .join("packages/tools/qlang.toml")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            &format!(
                "created: {}",
                project_root
                    .join("packages/tools/src/lib.ql")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            &format!(
                "created: {}",
                project_root
                    .join("packages/tools/src/main.ql")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            &format!(
                "created: {}",
                project_root
                    .join("packages/tools/tests/smoke.ql")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
        ],
    )
    .unwrap();

    let workspace_manifest = read_normalized_file(
        &project_root.join("qlang.toml"),
        "workspace manifest after add",
    );
    assert!(
        workspace_manifest.contains("packages/app"),
        "workspace manifest should keep existing member entry"
    );
    assert!(
        workspace_manifest.contains("packages/tools"),
        "workspace manifest should add the new member entry"
    );
    assert_eq!(
        read_normalized_file(
            &project_root.join("packages/tools/qlang.toml"),
            "added workspace member manifest"
        ),
        "[package]\nname = \"tools\"\n\n[dependencies]\napp = \"../app\"\ncore = \"../core\"\n"
    );
    assert_eq!(
        read_normalized_file(
            &project_root.join("packages/tools/src/lib.ql"),
            "added workspace member lib"
        ),
        "pub fn run() -> Int {\n    return 0\n}\n"
    );
    assert_eq!(
        read_normalized_file(
            &project_root.join("packages/tools/tests/smoke.ql"),
            "added workspace member smoke test"
        ),
        "fn main() -> Int {\n    return 0\n}\n"
    );

    let mut graph = ql_command(&workspace_root);
    graph.args(["project", "graph", &project_root.to_string_lossy()]);
    let output = run_command_capture(&mut graph, "`ql project graph` after add");
    let (stdout, stderr) =
        expect_success("project-add-success", "graph workspace after add", &output).unwrap();
    expect_empty_stderr("project-add-success", "graph workspace after add", &stderr).unwrap();
    expect_stdout_contains_all(
        "project-add-success",
        &stdout,
        &[
            "workspace_members:",
            "  - packages/app",
            "  - packages/core",
            "  - packages/tools",
            "  - member: packages/core",
            "    package: core",
            "  - member: packages/tools",
            "    package: tools",
            "    status: missing",
            "    references:",
            "      - ../app",
            "      - ../core",
        ],
    )
    .unwrap();
}

#[test]
fn project_add_refuses_duplicate_workspace_package_name() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-add-duplicate");
    let project_root = temp.path().join("workspace");

    let mut init = ql_command(&workspace_root);
    init.args([
        "project",
        "init",
        &project_root.to_string_lossy(),
        "--workspace",
        "--name",
        "app",
    ]);
    let output = run_command_capture(&mut init, "`ql project init` workspace for duplicate add");
    let (_stdout, stderr) = expect_success(
        "project-add-duplicate",
        "workspace init for duplicate add",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-add-duplicate",
        "workspace init for duplicate add",
        &stderr,
    )
    .unwrap();

    let mut add = ql_command(&workspace_root);
    add.args([
        "project",
        "add",
        &project_root.to_string_lossy(),
        "--name",
        "app",
    ]);
    let output = run_command_capture(&mut add, "`ql project add` duplicate package");
    let (stdout, stderr) = expect_exit_code(
        "project-add-duplicate",
        "duplicate workspace package add",
        &output,
        1,
    )
    .unwrap();
    expect_empty_stdout(
        "project-add-duplicate",
        "duplicate workspace package add",
        &stdout,
    )
    .unwrap();
    expect_stderr_contains(
        "project-add-duplicate",
        "duplicate workspace package add",
        &stderr,
        "already declares member `packages/app`",
    )
    .unwrap();
}

#[test]
fn project_add_refuses_to_overwrite_existing_member_directory() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-add-conflict");
    let project_root = temp.path().join("workspace");

    let mut init = ql_command(&workspace_root);
    init.args([
        "project",
        "init",
        &project_root.to_string_lossy(),
        "--workspace",
        "--name",
        "app",
    ]);
    let output = run_command_capture(&mut init, "`ql project init` workspace for conflict add");
    let (_stdout, stderr) = expect_success(
        "project-add-conflict",
        "workspace init for conflict add",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-add-conflict",
        "workspace init for conflict add",
        &stderr,
    )
    .unwrap();

    temp.write("workspace/packages/tools/placeholder.txt", "already-here");

    let mut add = ql_command(&workspace_root);
    add.args([
        "project",
        "add",
        &project_root.to_string_lossy(),
        "--name",
        "tools",
    ]);
    let output = run_command_capture(&mut add, "`ql project add` conflicting member directory");
    let (stdout, stderr) = expect_exit_code(
        "project-add-conflict",
        "conflicting workspace member add",
        &output,
        1,
    )
    .unwrap();
    expect_empty_stdout(
        "project-add-conflict",
        "conflicting workspace member add",
        &stdout,
    )
    .unwrap();
    expect_stderr_contains(
        "project-add-conflict",
        "conflicting workspace member add",
        &stderr,
        &format!(
            "error: `ql project add` would overwrite existing path `{}`",
            project_root
                .join("packages/tools")
                .to_string_lossy()
                .replace('\\', "/")
        ),
    )
    .unwrap();
}

#[test]
fn project_add_existing_workspace_member_from_source_path() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-add-existing");
    let project_root = temp.path().join("workspace");
    let existing_request_path = project_root.join("vendor/core/src/main.ql");

    let mut init = ql_command(&workspace_root);
    init.args([
        "project",
        "init",
        &project_root.to_string_lossy(),
        "--workspace",
        "--name",
        "app",
    ]);
    let output = run_command_capture(&mut init, "`ql project init` workspace for existing add");
    let (_stdout, stderr) = expect_success(
        "project-add-existing",
        "workspace init for existing add",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-add-existing",
        "workspace init for existing add",
        &stderr,
    )
    .unwrap();

    temp.write(
        "workspace/vendor/core/qlang.toml",
        "[package]\nname = \"core\"\n",
    );
    temp.write(
        "workspace/vendor/core/src/main.ql",
        "fn main() -> Int {\n    return 0\n}\n",
    );

    let mut add = ql_command(&workspace_root);
    add.args([
        "project",
        "add",
        &project_root.to_string_lossy(),
        "--existing",
        &existing_request_path.to_string_lossy(),
    ]);
    let output = run_command_capture(&mut add, "`ql project add --existing` source path");
    let (stdout, stderr) = expect_success(
        "project-add-existing",
        "add existing workspace member from source path",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-add-existing",
        "add existing workspace member from source path",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "project-add-existing",
        &stdout.replace('\\', "/"),
        &[
            &format!(
                "updated: {}",
                project_root
                    .join("qlang.toml")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            &format!(
                "added: {}",
                project_root
                    .join("vendor/core")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
        ],
    )
    .unwrap();

    assert_eq!(
        read_normalized_file(
            &project_root.join("qlang.toml"),
            "workspace manifest after existing member add"
        ),
        "[workspace]\nmembers = [\"packages/app\", \"vendor/core\"]\n"
    );
    assert_eq!(
        read_normalized_file(
            &project_root.join("vendor/core/qlang.toml"),
            "existing package manifest after workspace add"
        ),
        "[package]\nname = \"core\"\n"
    );
}

#[test]
fn project_add_existing_refuses_name_override() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-add-existing-name");
    let project_root = temp.path().join("workspace");
    let existing_member_root = project_root.join("vendor/core");

    let mut init = ql_command(&workspace_root);
    init.args([
        "project",
        "init",
        &project_root.to_string_lossy(),
        "--workspace",
        "--name",
        "app",
    ]);
    let output = run_command_capture(
        &mut init,
        "`ql project init` workspace for existing add name conflict",
    );
    let (_stdout, stderr) = expect_success(
        "project-add-existing-name",
        "workspace init for existing add name conflict",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-add-existing-name",
        "workspace init for existing add name conflict",
        &stderr,
    )
    .unwrap();

    temp.write(
        "workspace/vendor/core/qlang.toml",
        "[package]\nname = \"core\"\n",
    );

    let mut add = ql_command(&workspace_root);
    add.args([
        "project",
        "add",
        &project_root.to_string_lossy(),
        "--existing",
        &existing_member_root.to_string_lossy(),
        "--name",
        "core",
    ]);
    let output = run_command_capture(&mut add, "`ql project add --existing --name`");
    let (stdout, stderr) = expect_exit_code(
        "project-add-existing-name",
        "add existing workspace member with explicit name override",
        &output,
        1,
    )
    .unwrap();
    expect_empty_stdout(
        "project-add-existing-name",
        "add existing workspace member with explicit name override",
        &stdout,
    )
    .unwrap();
    expect_stderr_contains(
        "project-add-existing-name",
        "add existing workspace member with explicit name override",
        &stderr,
        "error: `ql project add --existing` does not accept `--name`; package name comes from the existing manifest",
    )
    .unwrap();
}

#[test]
fn project_add_refuses_unknown_workspace_dependency() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-add-missing-dependency");
    let project_root = temp.path().join("workspace");

    let mut init = ql_command(&workspace_root);
    init.args([
        "project",
        "init",
        &project_root.to_string_lossy(),
        "--workspace",
        "--name",
        "app",
    ]);
    let output = run_command_capture(
        &mut init,
        "`ql project init` workspace for missing dependency add",
    );
    let (_stdout, stderr) = expect_success(
        "project-add-missing-dependency",
        "workspace init for missing dependency add",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-add-missing-dependency",
        "workspace init for missing dependency add",
        &stderr,
    )
    .unwrap();

    let mut add = ql_command(&workspace_root);
    add.args([
        "project",
        "add",
        &project_root.to_string_lossy(),
        "--name",
        "tools",
        "--dependency",
        "missing",
    ]);
    let output = run_command_capture(&mut add, "`ql project add` missing dependency");
    let (stdout, stderr) = expect_exit_code(
        "project-add-missing-dependency",
        "workspace member add with missing dependency",
        &output,
        1,
    )
    .unwrap();
    expect_empty_stdout(
        "project-add-missing-dependency",
        "workspace member add with missing dependency",
        &stdout,
    )
    .unwrap();
    expect_stderr_contains(
        "project-add-missing-dependency",
        "workspace member add with missing dependency",
        &stderr,
        &format!(
            "error: `ql project add` workspace manifest `{}` does not contain package `missing`",
            project_root
                .join("qlang.toml")
                .to_string_lossy()
                .replace('\\', "/")
        ),
    )
    .unwrap();
    assert!(
        !project_root.join("packages/tools").exists(),
        "missing dependency add should not create the new workspace member directory"
    );
}

#[test]
fn project_add_dependency_updates_existing_package_manifest_from_member_source_path() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-add-dependency-success");
    let project_root = temp.path().join("workspace");
    let request_path = project_root.join("packages/app/src/main.ql");

    let mut init = ql_command(&workspace_root);
    init.args([
        "project",
        "init",
        &project_root.to_string_lossy(),
        "--workspace",
        "--name",
        "app",
    ]);
    let output = run_command_capture(&mut init, "`ql project init` workspace for add-dependency");
    let (_stdout, stderr) = expect_success(
        "project-add-dependency-success",
        "workspace init for add-dependency",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-add-dependency-success",
        "workspace init for add-dependency",
        &stderr,
    )
    .unwrap();

    let mut add_core = ql_command(&workspace_root);
    add_core.args([
        "project",
        "add",
        &project_root.to_string_lossy(),
        "--name",
        "core",
    ]);
    let output = run_command_capture(
        &mut add_core,
        "`ql project add` workspace member for add-dependency",
    );
    let (_stdout, stderr) = expect_success(
        "project-add-dependency-success",
        "add workspace member for add-dependency",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-add-dependency-success",
        "add workspace member for add-dependency",
        &stderr,
    )
    .unwrap();

    let mut add_dependency = ql_command(&workspace_root);
    add_dependency.args([
        "project",
        "add-dependency",
        &request_path.to_string_lossy(),
        "--name",
        "core",
    ]);
    let output = run_command_capture(
        &mut add_dependency,
        "`ql project add-dependency` workspace member source path",
    );
    let (stdout, stderr) = expect_success(
        "project-add-dependency-success",
        "add dependency to existing package manifest",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-add-dependency-success",
        "add dependency to existing package manifest",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "project-add-dependency-success",
        &stdout,
        &[&format!(
            "updated: {}",
            project_root
                .join("packages/app/qlang.toml")
                .to_string_lossy()
                .replace('\\', "/")
        )],
    )
    .unwrap();

    assert_eq!(
        read_normalized_file(
            &project_root.join("packages/app/qlang.toml"),
            "workspace member manifest after add-dependency"
        ),
        "[dependencies]\ncore = \"../core\"\n\n[package]\nname = \"app\"\n"
    );
}

#[test]
fn project_add_dependency_supports_workspace_root_package_selector() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-add-dependency-selector");
    let project_root = temp.path().join("workspace");

    let mut init = ql_command(&workspace_root);
    init.args([
        "project",
        "init",
        &project_root.to_string_lossy(),
        "--workspace",
        "--name",
        "app",
    ]);
    let output = run_command_capture(
        &mut init,
        "`ql project init` workspace for selected add-dependency",
    );
    let (_stdout, stderr) = expect_success(
        "project-add-dependency-selector",
        "workspace init for selected add-dependency",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-add-dependency-selector",
        "workspace init for selected add-dependency",
        &stderr,
    )
    .unwrap();

    let mut add_core = ql_command(&workspace_root);
    add_core.args([
        "project",
        "add",
        &project_root.to_string_lossy(),
        "--name",
        "core",
    ]);
    let output = run_command_capture(
        &mut add_core,
        "`ql project add` workspace member for selected add-dependency",
    );
    let (_stdout, stderr) = expect_success(
        "project-add-dependency-selector",
        "add workspace member for selected add-dependency",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-add-dependency-selector",
        "add workspace member for selected add-dependency",
        &stderr,
    )
    .unwrap();

    let mut add_dependency = ql_command(&workspace_root);
    add_dependency.args([
        "project",
        "add-dependency",
        &project_root.to_string_lossy(),
        "--package",
        "app",
        "--name",
        "core",
    ]);
    let output = run_command_capture(
        &mut add_dependency,
        "`ql project add-dependency --package` workspace root",
    );
    let (stdout, stderr) = expect_success(
        "project-add-dependency-selector",
        "add dependency from workspace root with package selector",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-add-dependency-selector",
        "add dependency from workspace root with package selector",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "project-add-dependency-selector",
        &stdout,
        &[&format!(
            "updated: {}",
            project_root
                .join("packages/app/qlang.toml")
                .to_string_lossy()
                .replace('\\', "/")
        )],
    )
    .unwrap();

    assert_eq!(
        read_normalized_file(
            &project_root.join("packages/app/qlang.toml"),
            "workspace member manifest after selected add-dependency"
        ),
        "[dependencies]\ncore = \"../core\"\n\n[package]\nname = \"app\"\n"
    );
}

#[test]
fn project_add_dependency_supports_external_local_path() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-add-dependency-path");
    let project_root = temp.path().join("workspace");
    let vendor_source_path = project_root.join("vendor/core/src/lib.ql");

    let mut init = ql_command(&workspace_root);
    init.args([
        "project",
        "init",
        &project_root.to_string_lossy(),
        "--workspace",
        "--name",
        "app",
    ]);
    let output = run_command_capture(
        &mut init,
        "`ql project init` workspace for path add-dependency",
    );
    let (_stdout, stderr) = expect_success(
        "project-add-dependency-path",
        "workspace init for path add-dependency",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-add-dependency-path",
        "workspace init for path add-dependency",
        &stderr,
    )
    .unwrap();

    temp.write(
        "workspace/vendor/core/qlang.toml",
        "[package]\nname = \"vendor.core\"\n",
    );
    temp.write(
        "workspace/vendor/core/src/lib.ql",
        "pub fn helper() -> Int {\n    return 1\n}\n",
    );

    let mut add_dependency = ql_command(&workspace_root);
    add_dependency.args([
        "project",
        "add-dependency",
        &project_root.to_string_lossy(),
        "--package",
        "app",
        "--path",
        &vendor_source_path.to_string_lossy(),
    ]);
    let output = run_command_capture(
        &mut add_dependency,
        "`ql project add-dependency --path` external local package",
    );
    let (stdout, stderr) = expect_success(
        "project-add-dependency-path",
        "add external local dependency by path",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-add-dependency-path",
        "add external local dependency by path",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "project-add-dependency-path",
        &stdout,
        &[&format!(
            "updated: {}",
            project_root
                .join("packages/app/qlang.toml")
                .to_string_lossy()
                .replace('\\', "/")
        )],
    )
    .unwrap();

    assert_eq!(
        read_normalized_file(
            &project_root.join("packages/app/qlang.toml"),
            "workspace member manifest after path add-dependency"
        ),
        "[dependencies]\n\"vendor.core\" = \"../../vendor/core\"\n\n[package]\nname = \"app\"\n"
    );
}

#[test]
fn project_add_dependency_refuses_name_and_path_together() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-add-dependency-conflict");
    let project_root = temp.path().join("workspace");

    let mut init = ql_command(&workspace_root);
    init.args([
        "project",
        "init",
        &project_root.to_string_lossy(),
        "--workspace",
        "--name",
        "app",
    ]);
    let output = run_command_capture(
        &mut init,
        "`ql project init` workspace for conflicting add-dependency selectors",
    );
    let (_stdout, stderr) = expect_success(
        "project-add-dependency-conflict",
        "workspace init for conflicting add-dependency selectors",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-add-dependency-conflict",
        "workspace init for conflicting add-dependency selectors",
        &stderr,
    )
    .unwrap();

    let mut add_dependency = ql_command(&workspace_root);
    add_dependency.args([
        "project",
        "add-dependency",
        &project_root.to_string_lossy(),
        "--package",
        "app",
        "--name",
        "core",
        "--path",
        &project_root.join("vendor/core").to_string_lossy(),
    ]);
    let output = run_command_capture(
        &mut add_dependency,
        "`ql project add-dependency` conflicting selectors",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-add-dependency-conflict",
        "add dependency with conflicting selectors",
        &output,
        1,
    )
    .unwrap();
    expect_empty_stdout(
        "project-add-dependency-conflict",
        "add dependency with conflicting selectors",
        &stdout,
    )
    .unwrap();
    expect_stderr_contains(
        "project-add-dependency-conflict",
        "add dependency with conflicting selectors",
        &stderr,
        "error: `ql project add-dependency` accepts either `--name <package>` or `--path <file-or-dir>`, not both",
    )
    .unwrap();
}

#[test]
fn project_add_dependency_refuses_missing_workspace_package() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-add-dependency-missing");
    let project_root = temp.path().join("workspace");
    let request_path = project_root.join("packages/app/src/main.ql");

    let mut init = ql_command(&workspace_root);
    init.args([
        "project",
        "init",
        &project_root.to_string_lossy(),
        "--workspace",
        "--name",
        "app",
    ]);
    let output = run_command_capture(
        &mut init,
        "`ql project init` workspace for missing add-dependency",
    );
    let (_stdout, stderr) = expect_success(
        "project-add-dependency-missing",
        "workspace init for missing add-dependency",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-add-dependency-missing",
        "workspace init for missing add-dependency",
        &stderr,
    )
    .unwrap();

    let mut add_dependency = ql_command(&workspace_root);
    add_dependency.args([
        "project",
        "add-dependency",
        &request_path.to_string_lossy(),
        "--name",
        "core",
    ]);
    let output = run_command_capture(
        &mut add_dependency,
        "`ql project add-dependency` missing workspace package",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-add-dependency-missing",
        "add dependency with missing workspace package",
        &output,
        1,
    )
    .unwrap();
    expect_empty_stdout(
        "project-add-dependency-missing",
        "add dependency with missing workspace package",
        &stdout,
    )
    .unwrap();
    expect_stderr_contains(
        "project-add-dependency-missing",
        "add dependency with missing workspace package",
        &stderr.replace('\\', "/"),
        &format!(
            "error: `ql project add-dependency` workspace manifest `{}` does not contain package `core`",
            project_root.join("qlang.toml").to_string_lossy().replace('\\', "/")
        ),
    )
    .unwrap();
}

#[test]
fn project_remove_dependency_updates_existing_package_manifest() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-remove-dependency-success");
    let project_root = temp.path().join("workspace");
    let request_path = project_root.join("packages/app/src/main.ql");

    temp.write(
        "workspace/qlang.toml",
        "[workspace]\nmembers = [\"packages/app\", \"packages/core\"]\n",
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        "[package]\nname = \"app\"\n\n[dependencies]\ncore = \"../core\"\n",
    );
    temp.write(
        "workspace/packages/core/qlang.toml",
        "[package]\nname = \"core\"\n",
    );
    temp.write(
        "workspace/packages/app/src/main.ql",
        "fn main() -> Int {\n    return 0\n}\n",
    );

    let mut remove_dependency = ql_command(&workspace_root);
    remove_dependency.args([
        "project",
        "remove-dependency",
        &request_path.to_string_lossy(),
        "--name",
        "core",
    ]);
    let output = run_command_capture(
        &mut remove_dependency,
        "`ql project remove-dependency` existing package manifest",
    );
    let (stdout, stderr) = expect_success(
        "project-remove-dependency-success",
        "remove dependency from existing package manifest",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-remove-dependency-success",
        "remove dependency from existing package manifest",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "project-remove-dependency-success",
        &stdout,
        &[&format!(
            "updated: {}",
            project_root
                .join("packages/app/qlang.toml")
                .to_string_lossy()
                .replace('\\', "/")
        )],
    )
    .unwrap();

    assert_eq!(
        read_normalized_file(
            &project_root.join("packages/app/qlang.toml"),
            "workspace member manifest after remove-dependency"
        ),
        "[package]\nname = \"app\"\n"
    );
}

#[test]
fn project_remove_dependency_supports_workspace_root_package_selector() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-remove-dependency-selector");
    let project_root = temp.path().join("workspace");

    temp.write(
        "workspace/qlang.toml",
        "[workspace]\nmembers = [\"packages/app\", \"packages/core\"]\n",
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        "[package]\nname = \"app\"\n\n[dependencies]\ncore = \"../core\"\n",
    );
    temp.write(
        "workspace/packages/core/qlang.toml",
        "[package]\nname = \"core\"\n",
    );

    let mut remove_dependency = ql_command(&workspace_root);
    remove_dependency.args([
        "project",
        "remove-dependency",
        &project_root.to_string_lossy(),
        "--package",
        "app",
        "--name",
        "core",
    ]);
    let output = run_command_capture(
        &mut remove_dependency,
        "`ql project remove-dependency --package` workspace root",
    );
    let (stdout, stderr) = expect_success(
        "project-remove-dependency-selector",
        "remove dependency from workspace root with package selector",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-remove-dependency-selector",
        "remove dependency from workspace root with package selector",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "project-remove-dependency-selector",
        &stdout,
        &[&format!(
            "updated: {}",
            project_root
                .join("packages/app/qlang.toml")
                .to_string_lossy()
                .replace('\\', "/")
        )],
    )
    .unwrap();

    assert_eq!(
        read_normalized_file(
            &project_root.join("packages/app/qlang.toml"),
            "workspace member manifest after selected remove-dependency"
        ),
        "[package]\nname = \"app\"\n"
    );
}

#[test]
fn project_remove_dependency_removes_legacy_reference_entry() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-remove-dependency-legacy");
    let project_root = temp.path().join("workspace");
    let request_path = project_root.join("packages/app/src/main.ql");

    temp.write(
        "workspace/qlang.toml",
        "[workspace]\nmembers = [\"packages/app\", \"packages/core\"]\n",
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        "[package]\nname = \"app\"\n\n[references]\npackages = [\"../core\"]\n",
    );
    temp.write(
        "workspace/packages/core/qlang.toml",
        "[package]\nname = \"core\"\n",
    );
    temp.write(
        "workspace/packages/app/src/main.ql",
        "fn main() -> Int {\n    return 0\n}\n",
    );

    let mut remove_dependency = ql_command(&workspace_root);
    remove_dependency.args([
        "project",
        "remove-dependency",
        &request_path.to_string_lossy(),
        "--name",
        "core",
    ]);
    let output = run_command_capture(
        &mut remove_dependency,
        "`ql project remove-dependency` legacy reference entry",
    );
    let (stdout, stderr) = expect_success(
        "project-remove-dependency-legacy",
        "remove legacy reference dependency from existing package manifest",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-remove-dependency-legacy",
        "remove legacy reference dependency from existing package manifest",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "project-remove-dependency-legacy",
        &stdout,
        &[&format!(
            "updated: {}",
            project_root
                .join("packages/app/qlang.toml")
                .to_string_lossy()
                .replace('\\', "/")
        )],
    )
    .unwrap();

    assert_eq!(
        read_normalized_file(
            &project_root.join("packages/app/qlang.toml"),
            "workspace member manifest after legacy remove-dependency"
        ),
        "[package]\nname = \"app\"\n"
    );
}

#[test]
fn project_remove_dependency_all_refuses_package_selector() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-remove-dependency-all-package");
    let project_root = temp.path().join("workspace");

    temp.write(
        "workspace/qlang.toml",
        "[workspace]\nmembers = [\"packages/app\", \"packages/core\"]\n",
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        "[package]\nname = \"app\"\n\n[dependencies]\ncore = \"../core\"\n",
    );
    temp.write(
        "workspace/packages/core/qlang.toml",
        "[package]\nname = \"core\"\n",
    );

    let mut remove_dependency = ql_command(&workspace_root);
    remove_dependency.args([
        "project",
        "remove-dependency",
        &project_root.to_string_lossy(),
        "--package",
        "app",
        "--name",
        "core",
        "--all",
    ]);
    let output = run_command_capture(
        &mut remove_dependency,
        "`ql project remove-dependency --all --package` workspace root",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-remove-dependency-all-package",
        "remove dependency all with package selector",
        &output,
        1,
    )
    .unwrap();
    expect_empty_stdout(
        "project-remove-dependency-all-package",
        "remove dependency all with package selector",
        &stdout,
    )
    .unwrap();
    expect_stderr_contains(
        "project-remove-dependency-all-package",
        "remove dependency all with package selector",
        &stderr,
        "error: `ql project remove-dependency --all` does not accept `--package`; bulk cleanup already targets all dependents of `--name`",
    )
    .unwrap();
}

#[test]
fn project_remove_dependency_all_updates_all_workspace_dependents() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-remove-dependency-all");
    let project_root = temp.path().join("workspace");

    temp.write(
        "workspace/qlang.toml",
        "[workspace]\nmembers = [\"packages/app\", \"packages/tools\", \"packages/core\"]\n",
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        "[package]\nname = \"app\"\n\n[dependencies]\ncore = \"../core\"\n",
    );
    temp.write(
        "workspace/packages/tools/qlang.toml",
        "[package]\nname = \"tools\"\n\n[references]\npackages = [\"../core\"]\n",
    );
    temp.write(
        "workspace/packages/core/qlang.toml",
        "[package]\nname = \"core\"\n",
    );

    let mut remove_dependency = ql_command(&workspace_root);
    remove_dependency.args([
        "project",
        "remove-dependency",
        &project_root.to_string_lossy(),
        "--name",
        "core",
        "--all",
    ]);
    let output = run_command_capture(
        &mut remove_dependency,
        "`ql project remove-dependency --all` workspace dependents",
    );
    let (stdout, stderr) = expect_success(
        "project-remove-dependency-all",
        "remove dependency from all workspace dependents",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-remove-dependency-all",
        "remove dependency from all workspace dependents",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "project-remove-dependency-all",
        &stdout.replace('\\', "/"),
        &[
            &format!(
                "updated: {}",
                project_root
                    .join("packages/app/qlang.toml")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            &format!(
                "updated: {}",
                project_root
                    .join("packages/tools/qlang.toml")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
        ],
    )
    .unwrap();

    assert_eq!(
        read_normalized_file(
            &project_root.join("packages/app/qlang.toml"),
            "workspace app manifest after remove-dependency --all"
        ),
        "[package]\nname = \"app\"\n"
    );
    assert_eq!(
        read_normalized_file(
            &project_root.join("packages/tools/qlang.toml"),
            "workspace tools manifest after remove-dependency --all"
        ),
        "[package]\nname = \"tools\"\n"
    );
}

#[test]
fn project_remove_dependency_all_derives_package_name_from_member_source_path() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-remove-dependency-all-derived-name");
    let project_root = temp.path().join("workspace");
    let request_path = project_root.join("packages/core/src/main.ql");

    temp.write(
        "workspace/qlang.toml",
        "[workspace]\nmembers = [\"packages/app\", \"packages/tools\", \"packages/core\"]\n",
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        "[package]\nname = \"app\"\n\n[dependencies]\ncore = \"../core\"\n",
    );
    temp.write(
        "workspace/packages/tools/qlang.toml",
        "[package]\nname = \"tools\"\n\n[references]\npackages = [\"../core\"]\n",
    );
    temp.write(
        "workspace/packages/core/qlang.toml",
        "[package]\nname = \"core\"\n",
    );
    temp.write(
        "workspace/packages/core/src/main.ql",
        "fn main() -> Int {\n    return 0\n}\n",
    );

    let mut remove_dependency = ql_command(&workspace_root);
    remove_dependency.args([
        "project",
        "remove-dependency",
        &request_path.to_string_lossy(),
        "--all",
    ]);
    let output = run_command_capture(
        &mut remove_dependency,
        "`ql project remove-dependency --all` derived package name",
    );
    let (stdout, stderr) = expect_success(
        "project-remove-dependency-all-derived-name",
        "remove dependency from all workspace dependents with derived package name",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-remove-dependency-all-derived-name",
        "remove dependency from all workspace dependents with derived package name",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "project-remove-dependency-all-derived-name",
        &stdout.replace('\\', "/"),
        &[
            &format!(
                "updated: {}",
                project_root
                    .join("packages/app/qlang.toml")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            &format!(
                "updated: {}",
                project_root
                    .join("packages/tools/qlang.toml")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
        ],
    )
    .unwrap();

    assert_eq!(
        read_normalized_file(
            &project_root.join("packages/app/qlang.toml"),
            "workspace app manifest after derived remove-dependency --all"
        ),
        "[package]\nname = \"app\"\n"
    );
    assert_eq!(
        read_normalized_file(
            &project_root.join("packages/tools/qlang.toml"),
            "workspace tools manifest after derived remove-dependency --all"
        ),
        "[package]\nname = \"tools\"\n"
    );
}

#[test]
fn project_remove_dependency_all_requires_name_for_workspace_root() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-remove-dependency-all-derived-name-missing");
    let project_root = temp.path().join("workspace");

    temp.write(
        "workspace/qlang.toml",
        "[workspace]\nmembers = [\"packages/app\", \"packages/core\"]\n",
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        "[package]\nname = \"app\"\n\n[dependencies]\ncore = \"../core\"\n",
    );
    temp.write(
        "workspace/packages/core/qlang.toml",
        "[package]\nname = \"core\"\n",
    );

    let mut remove_dependency = ql_command(&workspace_root);
    remove_dependency.args([
        "project",
        "remove-dependency",
        &project_root.to_string_lossy(),
        "--all",
    ]);
    let output = run_command_capture(
        &mut remove_dependency,
        "`ql project remove-dependency --all` ambiguous workspace root",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-remove-dependency-all-derived-name-missing",
        "remove dependency from all workspace dependents with ambiguous workspace root",
        &output,
        1,
    )
    .unwrap();
    expect_empty_stdout(
        "project-remove-dependency-all-derived-name-missing",
        "remove dependency from all workspace dependents with ambiguous workspace root",
        &stdout,
    )
    .unwrap();
    expect_stderr_contains(
        "project-remove-dependency-all-derived-name-missing",
        "remove dependency from all workspace dependents with ambiguous workspace root",
        &stderr.replace('\\', "/"),
        &format!(
            "error: `ql project remove-dependency --all` could not derive a package name from `{}`; rerun with `--name <package>`",
            project_root.to_string_lossy().replace('\\', "/")
        ),
    )
    .unwrap();
}

#[test]
fn project_remove_dependency_all_refuses_workspace_package_without_dependents() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-remove-dependency-all-empty");
    let project_root = temp.path().join("workspace");

    temp.write(
        "workspace/qlang.toml",
        "[workspace]\nmembers = [\"packages/app\", \"packages/core\"]\n",
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        "[package]\nname = \"app\"\n",
    );
    temp.write(
        "workspace/packages/core/qlang.toml",
        "[package]\nname = \"core\"\n",
    );

    let mut remove_dependency = ql_command(&workspace_root);
    remove_dependency.args([
        "project",
        "remove-dependency",
        &project_root.to_string_lossy(),
        "--name",
        "core",
        "--all",
    ]);
    let output = run_command_capture(
        &mut remove_dependency,
        "`ql project remove-dependency --all` package without dependents",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-remove-dependency-all-empty",
        "remove dependency from workspace package without dependents",
        &output,
        1,
    )
    .unwrap();
    expect_empty_stdout(
        "project-remove-dependency-all-empty",
        "remove dependency from workspace package without dependents",
        &stdout,
    )
    .unwrap();
    expect_stderr_contains(
        "project-remove-dependency-all-empty",
        "remove dependency from workspace package without dependents",
        &stderr.replace('\\', "/"),
        &format!(
            "error: `ql project remove-dependency` workspace package `core` does not have any dependent members to update in workspace manifest `{}`",
            project_root.join("qlang.toml").to_string_lossy().replace('\\', "/")
        ),
    )
    .unwrap();
}

#[test]
fn project_remove_updates_workspace_members_from_member_source_path() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-remove-success");
    let project_root = temp.path().join("workspace");
    let request_path = project_root.join("packages/tools/src/main.ql");
    let removed_member_root = project_root.join("packages/tools");

    let mut init = ql_command(&workspace_root);
    init.args([
        "project",
        "init",
        &project_root.to_string_lossy(),
        "--workspace",
        "--name",
        "app",
    ]);
    let output = run_command_capture(&mut init, "`ql project init` workspace for remove");
    let (_stdout, stderr) = expect_success(
        "project-remove-success",
        "workspace init for remove",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-remove-success",
        "workspace init for remove",
        &stderr,
    )
    .unwrap();

    let mut add_core = ql_command(&workspace_root);
    add_core.args([
        "project",
        "add",
        &project_root.to_string_lossy(),
        "--name",
        "core",
    ]);
    let output = run_command_capture(&mut add_core, "`ql project add` core for remove");
    let (_stdout, stderr) = expect_success(
        "project-remove-success",
        "workspace core add for remove",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-remove-success",
        "workspace core add for remove",
        &stderr,
    )
    .unwrap();

    let mut add_tools = ql_command(&workspace_root);
    add_tools.args([
        "project",
        "add",
        &project_root.to_string_lossy(),
        "--name",
        "tools",
        "--dependency",
        "app",
        "--dependency",
        "core",
    ]);
    let output = run_command_capture(&mut add_tools, "`ql project add` tools for remove");
    let (_stdout, stderr) = expect_success(
        "project-remove-success",
        "workspace tools add for remove",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-remove-success",
        "workspace tools add for remove",
        &stderr,
    )
    .unwrap();

    let mut remove = ql_command(&workspace_root);
    remove.args([
        "project",
        "remove",
        &request_path.to_string_lossy(),
        "--name",
        "tools",
    ]);
    let output = run_command_capture(
        &mut remove,
        "`ql project remove` workspace member source path",
    );
    let (stdout, stderr) =
        expect_success("project-remove-success", "remove workspace member", &output).unwrap();
    expect_empty_stderr("project-remove-success", "remove workspace member", &stderr).unwrap();
    expect_stdout_contains_all(
        "project-remove-success",
        &stdout.replace('\\', "/"),
        &[
            &format!(
                "updated: {}",
                project_root
                    .join("qlang.toml")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            &format!(
                "removed: {}",
                removed_member_root.to_string_lossy().replace('\\', "/")
            ),
        ],
    )
    .unwrap();

    let workspace_manifest = read_normalized_file(
        &project_root.join("qlang.toml"),
        "workspace manifest after remove",
    );
    assert!(
        workspace_manifest.contains("packages/app"),
        "workspace manifest should keep existing members after remove"
    );
    assert!(
        workspace_manifest.contains("packages/core"),
        "workspace manifest should keep unrelated members after remove"
    );
    assert!(
        !workspace_manifest.contains("packages/tools"),
        "workspace manifest should drop the removed member entry"
    );
    assert!(
        removed_member_root.is_dir(),
        "project remove should keep the removed member files on disk"
    );

    let mut graph = ql_command(&workspace_root);
    graph.args(["project", "graph", &project_root.to_string_lossy()]);
    let output = run_command_capture(&mut graph, "`ql project graph` after remove");
    let (stdout, stderr) = expect_success(
        "project-remove-success",
        "graph workspace after remove",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-remove-success",
        "graph workspace after remove",
        &stderr,
    )
    .unwrap();
    let normalized_stdout = stdout.replace('\\', "/");
    expect_stdout_contains_all(
        "project-remove-success",
        &normalized_stdout,
        &[
            "workspace_members:",
            "  - packages/app",
            "  - packages/core",
        ],
    )
    .unwrap();
    assert!(
        !normalized_stdout.contains("packages/tools"),
        "workspace graph should not include the removed member, got:\n{stdout}"
    );
}

#[test]
fn project_remove_cascade_updates_dependents_and_workspace_members() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-remove-cascade");
    let project_root = temp.path().join("workspace");
    let request_path = project_root.join("packages/core/src/main.ql");
    let removed_member_root = project_root.join("packages/core");

    let mut init = ql_command(&workspace_root);
    init.args([
        "project",
        "init",
        &project_root.to_string_lossy(),
        "--workspace",
        "--name",
        "app",
    ]);
    let output = run_command_capture(&mut init, "`ql project init` workspace for cascade remove");
    let (_stdout, stderr) = expect_success(
        "project-remove-cascade",
        "workspace init for cascade remove",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-remove-cascade",
        "workspace init for cascade remove",
        &stderr,
    )
    .unwrap();

    let mut add_core = ql_command(&workspace_root);
    add_core.args([
        "project",
        "add",
        &project_root.to_string_lossy(),
        "--name",
        "core",
    ]);
    let output = run_command_capture(&mut add_core, "`ql project add` core for cascade remove");
    let (_stdout, stderr) = expect_success(
        "project-remove-cascade",
        "workspace core add for cascade remove",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-remove-cascade",
        "workspace core add for cascade remove",
        &stderr,
    )
    .unwrap();

    let mut add_tools = ql_command(&workspace_root);
    add_tools.args([
        "project",
        "add",
        &project_root.to_string_lossy(),
        "--name",
        "tools",
        "--dependency",
        "core",
    ]);
    let output = run_command_capture(&mut add_tools, "`ql project add` tools for cascade remove");
    let (_stdout, stderr) = expect_success(
        "project-remove-cascade",
        "workspace tools add for cascade remove",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-remove-cascade",
        "workspace tools add for cascade remove",
        &stderr,
    )
    .unwrap();

    let mut remove = ql_command(&workspace_root);
    remove.args([
        "project",
        "remove",
        &request_path.to_string_lossy(),
        "--name",
        "core",
        "--cascade",
    ]);
    let output = run_command_capture(
        &mut remove,
        "`ql project remove --cascade` workspace member with dependents",
    );
    let (stdout, stderr) = expect_success(
        "project-remove-cascade",
        "remove workspace member with cascading dependency cleanup",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-remove-cascade",
        "remove workspace member with cascading dependency cleanup",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "project-remove-cascade",
        &stdout.replace('\\', "/"),
        &[
            &format!(
                "updated: {}",
                project_root
                    .join("qlang.toml")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            &format!(
                "updated: {}",
                project_root
                    .join("packages/tools/qlang.toml")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            &format!(
                "removed: {}",
                removed_member_root.to_string_lossy().replace('\\', "/")
            ),
        ],
    )
    .unwrap();

    let workspace_manifest = read_normalized_file(
        &project_root.join("qlang.toml"),
        "workspace manifest after cascade remove",
    );
    assert!(
        workspace_manifest.contains("packages/app"),
        "workspace manifest should keep unrelated members after cascade remove"
    );
    assert!(
        workspace_manifest.contains("packages/tools"),
        "workspace manifest should keep dependents after cascade remove"
    );
    assert!(
        !workspace_manifest.contains("packages/core"),
        "workspace manifest should drop the removed member entry after cascade remove"
    );
    assert_eq!(
        read_normalized_file(
            &project_root.join("packages/tools/qlang.toml"),
            "dependent manifest after cascade remove"
        ),
        "[package]\nname = \"tools\"\n"
    );
    assert!(
        removed_member_root.is_dir(),
        "project remove --cascade should keep the removed member files on disk"
    );
}

#[test]
fn project_remove_refuses_workspace_member_with_dependents() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-remove-dependent");
    let project_root = temp.path().join("workspace");
    let request_path = project_root.join("packages/core/src/main.ql");

    let mut init = ql_command(&workspace_root);
    init.args([
        "project",
        "init",
        &project_root.to_string_lossy(),
        "--workspace",
        "--name",
        "app",
    ]);
    let output = run_command_capture(
        &mut init,
        "`ql project init` workspace for dependent remove",
    );
    let (_stdout, stderr) = expect_success(
        "project-remove-dependent",
        "workspace init for dependent remove",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-remove-dependent",
        "workspace init for dependent remove",
        &stderr,
    )
    .unwrap();

    let mut add_core = ql_command(&workspace_root);
    add_core.args([
        "project",
        "add",
        &project_root.to_string_lossy(),
        "--name",
        "core",
    ]);
    let output = run_command_capture(&mut add_core, "`ql project add` core for dependent remove");
    let (_stdout, stderr) = expect_success(
        "project-remove-dependent",
        "workspace core add for dependent remove",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-remove-dependent",
        "workspace core add for dependent remove",
        &stderr,
    )
    .unwrap();

    let mut add_tools = ql_command(&workspace_root);
    add_tools.args([
        "project",
        "add",
        &project_root.to_string_lossy(),
        "--name",
        "tools",
        "--dependency",
        "core",
    ]);
    let output = run_command_capture(
        &mut add_tools,
        "`ql project add` tools for dependent remove",
    );
    let (_stdout, stderr) = expect_success(
        "project-remove-dependent",
        "workspace tools add for dependent remove",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-remove-dependent",
        "workspace tools add for dependent remove",
        &stderr,
    )
    .unwrap();

    let mut remove = ql_command(&workspace_root);
    remove.args([
        "project",
        "remove",
        &request_path.to_string_lossy(),
        "--name",
        "core",
    ]);
    let output = run_command_capture(
        &mut remove,
        "`ql project remove` workspace member with dependents",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-remove-dependent",
        "remove workspace member with dependents",
        &output,
        1,
    )
    .unwrap();
    expect_empty_stdout(
        "project-remove-dependent",
        "remove workspace member with dependents",
        &stdout,
    )
    .unwrap();
    expect_stderr_contains(
        "project-remove-dependent",
        "remove workspace member with dependents",
        &stderr.replace('\\', "/"),
        &format!(
            "error: `ql project remove` cannot remove member package `core` from workspace manifest `{}` because other members still depend on it: packages/tools (tools); remove those edges first with `ql project remove-dependency <member> --name core` or rerun with `ql project remove <file-or-dir> --name core --cascade`",
            project_root.join("qlang.toml").to_string_lossy().replace('\\', "/")
        ),
    )
    .unwrap();

    let workspace_manifest = read_normalized_file(
        &project_root.join("qlang.toml"),
        "workspace manifest after refused dependent remove",
    );
    assert!(
        workspace_manifest.contains("packages/core"),
        "workspace manifest should keep dependent member after refused remove"
    );
    assert!(
        project_root.join("packages/core").is_dir(),
        "refused remove should keep member directory on disk"
    );
}

#[test]
fn project_remove_refuses_unknown_workspace_member_package() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-remove-missing");
    let project_root = temp.path().join("workspace");

    let mut init = ql_command(&workspace_root);
    init.args([
        "project",
        "init",
        &project_root.to_string_lossy(),
        "--workspace",
        "--name",
        "app",
    ]);
    let output = run_command_capture(&mut init, "`ql project init` workspace for missing remove");
    let (_stdout, stderr) = expect_success(
        "project-remove-missing",
        "workspace init for missing remove",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-remove-missing",
        "workspace init for missing remove",
        &stderr,
    )
    .unwrap();

    let mut remove = ql_command(&workspace_root);
    remove.args([
        "project",
        "remove",
        &project_root.to_string_lossy(),
        "--name",
        "tools",
    ]);
    let output = run_command_capture(&mut remove, "`ql project remove` missing package");
    let (stdout, stderr) = expect_exit_code(
        "project-remove-missing",
        "remove missing workspace member package",
        &output,
        1,
    )
    .unwrap();
    expect_empty_stdout(
        "project-remove-missing",
        "remove missing workspace member package",
        &stdout,
    )
    .unwrap();
    expect_stderr_contains(
        "project-remove-missing",
        "remove missing workspace member package",
        &stderr.replace('\\', "/"),
        &format!(
            "error: `ql project remove` workspace manifest `{}` does not contain member package `tools`",
            project_root.join("qlang.toml").to_string_lossy().replace('\\', "/")
        ),
    )
    .unwrap();
}

#[test]
fn project_remove_rejects_ambiguous_workspace_member_package_names() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-remove-ambiguous");
    let project_root = temp.path().join("workspace");

    temp.write(
        "workspace/qlang.toml",
        "[workspace]\nmembers = [\"packages/a\", \"packages/b\"]\n",
    );
    temp.write(
        "workspace/packages/a/qlang.toml",
        "[package]\nname = \"util\"\n",
    );
    temp.write(
        "workspace/packages/b/qlang.toml",
        "[package]\nname = \"util\"\n",
    );

    let mut remove = ql_command(&workspace_root);
    remove.args([
        "project",
        "remove",
        &project_root.to_string_lossy(),
        "--name",
        "util",
    ]);
    let output = run_command_capture(&mut remove, "`ql project remove` ambiguous package");
    let (stdout, stderr) = expect_exit_code(
        "project-remove-ambiguous",
        "remove ambiguous workspace member package",
        &output,
        1,
    )
    .unwrap();
    expect_empty_stdout(
        "project-remove-ambiguous",
        "remove ambiguous workspace member package",
        &stdout,
    )
    .unwrap();
    expect_stderr_contains(
        "project-remove-ambiguous",
        "remove ambiguous workspace member package",
        &stderr.replace('\\', "/"),
        &format!(
            "error: `ql project remove` workspace manifest `{}` contains multiple members for package `util`: packages/a, packages/b",
            project_root.join("qlang.toml").to_string_lossy().replace('\\', "/")
        ),
    )
    .unwrap();
}
