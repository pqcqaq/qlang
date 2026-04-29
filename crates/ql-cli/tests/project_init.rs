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
use std.core.and_bool as and_bool
use std.core.clamp_bounds_int as clamp_bounds_int
use std.core.clamp_max_int as clamp_max_int
use std.core.clamp_min_int as clamp_min_int
use std.core.compare_int as compare_int
use std.core.distance_to_bounds_int as distance_to_bounds_int
use std.core.distance_to_range_int as distance_to_range_int
use std.core.implies_bool as implies_bool
use std.core.in_bounds_int as in_bounds_int
use std.core.in_exclusive_bounds_int as in_exclusive_bounds_int
use std.core.is_ascending_int as is_ascending_int
use std.core.is_descending_int as is_descending_int
use std.core.is_not_within_int as is_not_within_int
use std.core.is_outside_bounds_int as is_outside_bounds_int
use std.core.is_outside_range_int as is_outside_range_int
use std.core.is_strictly_descending_int as is_strictly_descending_int
use std.core.is_strictly_ascending_int as is_strictly_ascending_int
use std.core.is_within_int as is_within_int
use std.core.lower_bound_int as lower_bound_int
use std.core.max3_int as max3_int
use std.core.max_int as max_int
use std.core.median3_int as median3_int
use std.core.min3_int as min3_int
use std.core.range_span_int as range_span_int
use std.core.upper_bound_int as upper_bound_int
use std.core.xor_bool as xor_bool
use std.test.expect_bool_and as expect_bool_and
use std.test.expect_bool_eq as expect_bool_eq
use std.test.expect_bool_implies as expect_bool_implies
use std.test.expect_bool_ne as expect_bool_ne
use std.test.expect_bool_not as expect_bool_not
use std.test.expect_bool_or as expect_bool_or
use std.test.expect_bool_xor as expect_bool_xor
use std.test.expect_false as expect_false
use std.test.expect_status_failed as expect_status_failed
use std.test.expect_status_ok as expect_status_ok
use std.test.expect_int_ascending as expect_int_ascending
use std.test.expect_int_between as expect_int_between
use std.test.expect_int_between_bounds as expect_int_between_bounds
use std.test.expect_int_clamped as expect_int_clamped
use std.test.expect_int_clamped_bounds as expect_int_clamped_bounds
use std.test.expect_int_descending as expect_int_descending
use std.test.expect_int_distance_to_bounds as expect_int_distance_to_bounds
use std.test.expect_int_distance_to_range as expect_int_distance_to_range
use std.test.expect_int_divisible_by as expect_int_divisible_by
use std.test.expect_int_eq as expect_int_eq
use std.test.expect_int_even as expect_int_even
use std.test.expect_int_exclusive_between_bounds as expect_int_exclusive_between_bounds
use std.test.expect_int_exclusive_between as expect_int_exclusive_between
use std.test.expect_int_negative as expect_int_negative
use std.test.expect_int_not_within as expect_int_not_within
use std.test.expect_int_nonnegative as expect_int_nonnegative
use std.test.expect_int_nonpositive as expect_int_nonpositive
use std.test.expect_int_odd as expect_int_odd
use std.test.expect_int_outside as expect_int_outside
use std.test.expect_int_outside_bounds as expect_int_outside_bounds
use std.test.expect_int_positive as expect_int_positive
use std.test.expect_int_strictly_descending as expect_int_strictly_descending
use std.test.expect_int_strictly_ascending as expect_int_strictly_ascending
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
    let min3_check = expect_int_eq(min3_int(20, 22, 21), 20)
    let median3_check = expect_int_eq(median3_int(22, 20, 21), 21)
    let clamp_min_check = expect_int_eq(clamp_min_int(19, 20), 20)
    let clamp_max_check = expect_int_eq(clamp_max_int(23, 22), 22)
    let clamp_bounds_check = expect_int_eq(clamp_bounds_int(23, 22, 20), 22)
    let abs_diff_check = expect_int_eq(abs_diff_int(22, 19), 3)
    let range_span_check = expect_int_eq(range_span_int(22, 20), 2)
    let lower_bound_check = expect_int_eq(lower_bound_int(22, 20), 20)
    let upper_bound_check = expect_int_eq(upper_bound_int(22, 20), 22)
    let distance_range_check = expect_int_eq(distance_to_range_int(19, 20, 22), 1)
    let distance_bounds_check = expect_int_eq(distance_to_bounds_int(23, 22, 20), 1)
    let compare_check = expect_int_eq(compare_int(9, 3), 1)
    let and_check = expect_false(and_bool(true, false))
    let xor_check = expect_bool_eq(xor_bool(true, false), true)
    let bool_ne_check = expect_bool_ne(true, false)
    let bool_not_check = expect_bool_not(false, true)
    let bool_and_check = expect_bool_and(true, false, false)
    let bool_or_check = expect_bool_or(false, true, true)
    let bool_xor_check = expect_bool_xor(true, true, false)
    let core_implies_check = expect_bool_eq(implies_bool(true, false), false)
    let core_ascending_check = expect_bool_eq(is_ascending_int(20, 21, 22), true)
    let core_strict_ascending_check = expect_bool_eq(is_strictly_ascending_int(20, 20, 22), false)
    let core_descending_check = expect_bool_eq(is_descending_int(22, 21, 20), true)
    let core_strict_descending_check = expect_bool_eq(is_strictly_descending_int(22, 22, 20), false)
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
    let clamped_check = expect_int_clamped(19, 20, 22, 20)
    let clamped_bounds_check = expect_int_clamped_bounds(23, 22, 20, 22)
    let distance_range_expect_check = expect_int_distance_to_range(19, 20, 22, 1)
    let distance_bounds_expect_check = expect_int_distance_to_bounds(23, 22, 20, 1)
    let ascending_check = expect_int_ascending(20, 21, 22)
    let strict_ascending_check = expect_int_strictly_ascending(20, 21, 22)
    let descending_check = expect_int_descending(22, 21, 20)
    let strict_descending_check = expect_int_strictly_descending(22, 21, 20)
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
    let failed_range_check = expect_int_eq(expect_int_between(19, 20, 22), 1)
    let failed_exclusive_range_check = expect_int_eq(expect_int_exclusive_between(20, 20, 22), 1)
    let failed_outside_check = expect_int_eq(expect_int_outside(21, 20, 22), 1)
    let failed_bounds_check = expect_int_eq(expect_int_between_bounds(19, 22, 20), 1)
    let failed_exclusive_bounds_check = expect_int_eq(expect_int_exclusive_between_bounds(22, 22, 20), 1)
    let failed_outside_bounds_check = expect_int_eq(expect_int_outside_bounds(21, 22, 20), 1)
    let failed_clamped_check = expect_int_eq(expect_int_clamped(19, 20, 22, 19), 1)
    let failed_clamped_bounds_check = expect_int_eq(expect_int_clamped_bounds(23, 22, 20, 23), 1)
    let failed_distance_range_check = expect_int_eq(expect_int_distance_to_range(21, 20, 22, 1), 1)
    let failed_distance_bounds_check = expect_int_eq(expect_int_distance_to_bounds(21, 22, 20, 1), 1)
    let failed_ascending_check = expect_int_eq(expect_int_ascending(22, 21, 20), 1)
    let failed_strict_ascending_check = expect_int_eq(expect_int_strictly_ascending(20, 20, 22), 1)
    let failed_descending_check = expect_int_eq(expect_int_descending(20, 22, 21), 1)
    let failed_strict_descending_check = expect_int_eq(expect_int_strictly_descending(22, 22, 20), 1)
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

    let core_status = merge_status6(max_check + max3_check + min3_check + median3_check, clamp_min_check + clamp_max_check + clamp_bounds_check + abs_diff_check, range_span_check + compare_check + and_check + xor_check, bool_ne_check + bool_not_check + bool_and_check + bool_or_check, core_descending_check + core_strict_descending_check + core_not_within_check + core_outside_range_check, core_outside_bounds_check + lower_bound_check + upper_bound_check + distance_range_check + distance_bounds_check)
    let bool_status = merge_status4(bool_xor_check + core_implies_check + core_ascending_check + core_strict_ascending_check, core_bounds_check + core_exclusive_bounds_check + core_within_check + range_check, failed_bool_ne_check + failed_bool_not_check + failed_bool_and_check + failed_bool_or_check, failed_bool_xor_check + exclusive_range_check + outside_check + bounds_check)
    let range_status = merge_status5(exclusive_bounds_check + outside_bounds_check + clamped_check + clamped_bounds_check, distance_range_expect_check + distance_bounds_expect_check + ascending_check + strict_ascending_check, descending_check + strict_descending_check + divisible_check + within_check, not_within_check + even_check + odd_check + positive_check, negative_check + nonnegative_check + nonpositive_check + test_implies_check + true_check + status_ok_bool_check)
    let status_helper_status = merge_status4(status_failed_bool_check + merged_status_check + merged_status3_check + merged_status4_check, merged_status5_check + merged_status6_check + status_ok_check + status_failed_check, failed_status_ok_check + failed_status_failed_check + failed_range_check + failed_exclusive_range_check, failed_outside_check + failed_bounds_check + failed_exclusive_bounds_check + failed_outside_bounds_check)
    let failure_status = merge_status4(failed_clamped_check + failed_clamped_bounds_check + failed_distance_range_check + failed_distance_bounds_check, failed_ascending_check + failed_strict_ascending_check + failed_descending_check + failed_strict_descending_check, failed_divisible_check + failed_within_check + failed_not_within_check + failed_even_check, failed_odd_check + failed_positive_check + failed_negative_check + failed_nonnegative_check + failed_nonpositive_check + failed_implies_check)

    return expect_status_ok(merge_status5(core_status, bool_status, range_status, status_helper_status, failure_status))
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
        "[package]\nname = \"demo-package\"\n\n[dependencies]\n\"std.core\" = \"../stdlib/packages/core\"\n\"std.test\" = \"../stdlib/packages/test\"\n"
    );
    assert_eq!(
        read_normalized_file(&project_root.join("src/lib.ql"), "stdlib package source"),
        "use std.core.clamp_int as clamp_int\n\npub fn run() -> Int {\n    return clamp_int(42, 0, 100)\n}\n"
    );
    assert_eq!(
        read_normalized_file(
            &project_root.join("tests/smoke.ql"),
            "stdlib package smoke test"
        ),
        expected_stdlib_package_smoke_source()
    );

    let mut check = ql_command(&workspace_root);
    check.args([
        "check",
        "--sync-interfaces",
        &project_root.to_string_lossy(),
    ]);
    let output = run_command_capture(&mut check, "`ql check --sync-interfaces` stdlib package");
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
        "[package]\nname = \"app\"\n\n[dependencies]\n\"std.core\" = \"../../../stdlib/packages/core\"\n\"std.test\" = \"../../../stdlib/packages/test\"\n"
    );
    assert_eq!(
        read_normalized_file(
            &member_root.join("src/lib.ql"),
            "stdlib workspace member source"
        ),
        "use std.core.clamp_int as clamp_int\n\npub fn run() -> Int {\n    return clamp_int(42, 0, 100)\n}\n"
    );
    assert_eq!(
        read_normalized_file(
            &member_root.join("tests/smoke.ql"),
            "stdlib workspace member smoke test"
        ),
        expected_stdlib_package_smoke_source()
    );

    let mut check = ql_command(&workspace_root);
    check.args([
        "check",
        "--sync-interfaces",
        &project_root.to_string_lossy(),
    ]);
    let output = run_command_capture(&mut check, "`ql check --sync-interfaces` stdlib workspace");
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
