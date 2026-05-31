mod support;

use support::codegen::{FailCase, run_fail_case};
use support::workspace_root;

#[test]
fn codegen_failure_cases_match() {
    let workspace_root = workspace_root();
    let mut failures = Vec::new();

    for case in codegen_fail_cases() {
        if let Err(message) = run_fail_case(&workspace_root, case) {
            failures.push(message);
        }
    }

    assert!(
        failures.is_empty(),
        "codegen failure regressions:\n\n{}",
        failures.join("\n\n")
    );
}

fn codegen_fail_cases() -> &'static [FailCase] {
    &[
        FailCase {
            name: "unsupported_extern_rust_abi_build",
            source_relative: "tests/codegen/fail/unsupported_extern_rust_abi_build.ql",
            emit: "llvm-ir",
            expected_stderr_relative: "tests/codegen/fail/unsupported_extern_rust_abi_build.stderr",
            extra_args: &[],
        },
        FailCase {
            name: "unsupported_extern_rust_abi_definition_build",
            source_relative: "tests/codegen/fail/unsupported_extern_rust_abi_definition_build.ql",
            emit: "llvm-ir",
            expected_stderr_relative: "tests/codegen/fail/unsupported_extern_rust_abi_definition_build.stderr",
            extra_args: &[],
        },
        FailCase {
            name: "unsupported_empty_array_without_expected_build",
            source_relative: "tests/codegen/fail/unsupported_empty_array_without_expected_build.ql",
            emit: "llvm-ir",
            expected_stderr_relative: "tests/codegen/fail/unsupported_empty_array_without_expected_build.stderr",
            extra_args: &[],
        },
        FailCase {
            name: "unsupported_for_build",
            source_relative: "tests/codegen/fail/unsupported_for_build.ql",
            emit: "llvm-ir",
            expected_stderr_relative: "tests/codegen/fail/unsupported_for_build.stderr",
            extra_args: &[],
        },
        FailCase {
            name: "unsupported_cleanup_for_build",
            source_relative: "tests/codegen/fail/unsupported_cleanup_for_build.ql",
            emit: "llvm-ir",
            expected_stderr_relative: "tests/codegen/fail/unsupported_cleanup_for_build.stderr",
            extra_args: &[],
        },
        FailCase {
            name: "unsupported_async_generic_main_build",
            source_relative: "tests/codegen/fail/unsupported_async_generic_main_build.ql",
            emit: "llvm-ir",
            expected_stderr_relative: "tests/codegen/fail/unsupported_async_generic_main_build.stderr",
            extra_args: &[],
        },
        FailCase {
            name: "unsupported_deferred_multi_segment_type_build",
            source_relative: "tests/codegen/fail/unsupported_deferred_multi_segment_type_build.ql",
            emit: "dylib",
            expected_stderr_relative: "tests/codegen/fail/unsupported_deferred_multi_segment_type_build.stderr",
            extra_args: &[],
        },
        FailCase {
            name: "dylib_requires_export_build",
            source_relative: "tests/codegen/fail/dylib_requires_export_build.ql",
            emit: "dylib",
            expected_stderr_relative: "tests/codegen/fail/dylib_requires_export_build.stderr",
            extra_args: &[],
        },
        FailCase {
            name: "executable_header_build",
            source_relative: "fixtures/codegen/pass/minimal_build.ql",
            emit: "exe",
            expected_stderr_relative: "tests/codegen/fail/executable_header_build.stderr",
            extra_args: &["--header"],
        },
    ]
}
