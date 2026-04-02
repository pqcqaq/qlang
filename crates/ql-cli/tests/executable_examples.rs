mod support;

use std::path::Path;
use std::process::Command;

use ql_driver::{ToolchainOptions, discover_toolchain};
use support::{
    TempDir, executable_output_path, expect_exit_code, expect_file_exists, expect_silent_output,
    expect_success, run_command_capture, run_ql_build_capture, workspace_root,
};

#[test]
fn executable_examples_build_and_run() {
    let workspace_root = workspace_root();
    let examples_root = workspace_root.join("ramdon_tests/executable_examples");
    assert!(
        examples_root.is_dir(),
        "expected sync executable examples under `{}`",
        examples_root.display()
    );

    if !toolchain_available("sync executable example test") {
        return;
    }

    let cases = [
        ExecutableExampleCase {
            name: "sync_minimal",
            source_relative: "ramdon_tests/executable_examples/01_sync_minimal.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "sync_data_shapes",
            source_relative: "ramdon_tests/executable_examples/02_sync_data_shapes.ql",
            expected_exit: 32,
        },
        ExecutableExampleCase {
            name: "sync_extern_c_export",
            source_relative: "ramdon_tests/executable_examples/03_sync_extern_c_export.ql",
            expected_exit: 42,
        },
    ];

    assert_example_cases_run(
        &workspace_root,
        &cases,
        "sync executable example regressions",
    );
}

#[test]
fn async_program_surface_examples_build_and_run() {
    let workspace_root = workspace_root();
    let examples_root = workspace_root.join("ramdon_tests/async_program_surface_examples");
    assert!(
        examples_root.is_dir(),
        "expected async program examples under `{}`",
        examples_root.display()
    );

    if !toolchain_available("async executable example test") {
        return;
    }

    let cases = [
        ExecutableExampleCase {
            name: "async_main_basics",
            source_relative: "ramdon_tests/async_program_surface_examples/04_async_main_basics.ql",
            expected_exit: 28,
        },
        ExecutableExampleCase {
            name: "async_main_aggregates_and_for_await",
            source_relative: "ramdon_tests/async_program_surface_examples/05_async_main_aggregates_and_for_await.ql",
            expected_exit: 71,
        },
        ExecutableExampleCase {
            name: "async_main_task_handle_payloads",
            source_relative: "ramdon_tests/async_program_surface_examples/06_async_main_task_handle_payloads.ql",
            expected_exit: 39,
        },
        ExecutableExampleCase {
            name: "async_main_projection_reinit",
            source_relative: "ramdon_tests/async_program_surface_examples/07_async_main_projection_reinit.ql",
            expected_exit: 45,
        },
        ExecutableExampleCase {
            name: "async_main_dynamic_task_arrays",
            source_relative: "ramdon_tests/async_program_surface_examples/08_async_main_dynamic_task_arrays.ql",
            expected_exit: 70,
        },
        ExecutableExampleCase {
            name: "async_main_zero_sized",
            source_relative: "ramdon_tests/async_program_surface_examples/09_async_main_zero_sized.ql",
            expected_exit: 10,
        },
        ExecutableExampleCase {
            name: "async_main_guard_refined_projected_root",
            source_relative: "ramdon_tests/async_program_surface_examples/10_async_main_guard_refined_projected_root.ql",
            expected_exit: 49,
        },
        ExecutableExampleCase {
            name: "async_main_const_backed_projected_root",
            source_relative: "ramdon_tests/async_program_surface_examples/11_async_main_const_backed_projected_root.ql",
            expected_exit: 24,
        },
        ExecutableExampleCase {
            name: "async_main_aliased_projected_root",
            source_relative: "ramdon_tests/async_program_surface_examples/12_async_main_aliased_projected_root.ql",
            expected_exit: 17,
        },
        ExecutableExampleCase {
            name: "async_main_aliased_const_backed_projected_root",
            source_relative: "ramdon_tests/async_program_surface_examples/13_async_main_aliased_const_backed_projected_root.ql",
            expected_exit: 17,
        },
        ExecutableExampleCase {
            name: "async_main_aliased_guard_refined_projected_root",
            source_relative: "ramdon_tests/async_program_surface_examples/14_async_main_aliased_guard_refined_projected_root.ql",
            expected_exit: 21,
        },
        ExecutableExampleCase {
            name: "async_main_aliased_guard_refined_const_backed_projected_root",
            source_relative: "ramdon_tests/async_program_surface_examples/15_async_main_aliased_guard_refined_const_backed_projected_root.ql",
            expected_exit: 25,
        },
        ExecutableExampleCase {
            name: "async_main_aliased_projected_root_tuple_repackage_reinit",
            source_relative: "ramdon_tests/async_program_surface_examples/16_async_main_aliased_projected_root_tuple_repackage_reinit.ql",
            expected_exit: 31,
        },
        ExecutableExampleCase {
            name: "async_main_aliased_projected_root_struct_repackage_reinit",
            source_relative: "ramdon_tests/async_program_surface_examples/17_async_main_aliased_projected_root_struct_repackage_reinit.ql",
            expected_exit: 32,
        },
        ExecutableExampleCase {
            name: "async_main_aliased_projected_root_nested_repackage_reinit",
            source_relative: "ramdon_tests/async_program_surface_examples/18_async_main_aliased_projected_root_nested_repackage_reinit.ql",
            expected_exit: 33,
        },
        ExecutableExampleCase {
            name: "async_main_aliased_guard_refined_const_backed_nested_repackage_reinit",
            source_relative: "ramdon_tests/async_program_surface_examples/19_async_main_aliased_guard_refined_const_backed_nested_repackage_reinit.ql",
            expected_exit: 36,
        },
        ExecutableExampleCase {
            name: "async_main_aliased_projected_root_nested_repackage_spawn",
            source_relative: "ramdon_tests/async_program_surface_examples/20_async_main_aliased_projected_root_nested_repackage_spawn.ql",
            expected_exit: 34,
        },
        ExecutableExampleCase {
            name: "async_main_aliased_guard_refined_const_backed_nested_repackage_spawn",
            source_relative: "ramdon_tests/async_program_surface_examples/21_async_main_aliased_guard_refined_const_backed_nested_repackage_spawn.ql",
            expected_exit: 38,
        },
        ExecutableExampleCase {
            name: "async_main_aliased_projected_root_array_repackage_spawn",
            source_relative: "ramdon_tests/async_program_surface_examples/22_async_main_aliased_projected_root_array_repackage_spawn.ql",
            expected_exit: 37,
        },
        ExecutableExampleCase {
            name: "async_main_aliased_guard_refined_const_backed_array_repackage_spawn",
            source_relative: "ramdon_tests/async_program_surface_examples/23_async_main_aliased_guard_refined_const_backed_array_repackage_spawn.ql",
            expected_exit: 40,
        },
        ExecutableExampleCase {
            name: "async_main_aliased_projected_root_nested_array_repackage_spawn",
            source_relative: "ramdon_tests/async_program_surface_examples/24_async_main_aliased_projected_root_nested_array_repackage_spawn.ql",
            expected_exit: 41,
        },
        ExecutableExampleCase {
            name: "async_main_aliased_guard_refined_const_backed_nested_array_repackage_spawn",
            source_relative: "ramdon_tests/async_program_surface_examples/25_async_main_aliased_guard_refined_const_backed_nested_array_repackage_spawn.ql",
            expected_exit: 46,
        },
        ExecutableExampleCase {
            name: "async_main_composed_dynamic_nested_array_repackage_spawn",
            source_relative: "ramdon_tests/async_program_surface_examples/26_async_main_composed_dynamic_nested_array_repackage_spawn.ql",
            expected_exit: 47,
        },
        ExecutableExampleCase {
            name: "async_main_alias_sourced_composed_dynamic_nested_array_repackage_spawn",
            source_relative: "ramdon_tests/async_program_surface_examples/27_async_main_alias_sourced_composed_dynamic_nested_array_repackage_spawn.ql",
            expected_exit: 48,
        },
        ExecutableExampleCase {
            name: "async_main_guarded_alias_sourced_composed_dynamic_nested_array_repackage_spawn",
            source_relative: "ramdon_tests/async_program_surface_examples/28_async_main_guarded_alias_sourced_composed_dynamic_nested_array_repackage_spawn.ql",
            expected_exit: 50,
        },
        ExecutableExampleCase {
            name: "async_main_aliased_projected_root_forwarded_nested_array_repackage_spawn",
            source_relative: "ramdon_tests/async_program_surface_examples/29_async_main_aliased_projected_root_forwarded_nested_array_repackage_spawn.ql",
            expected_exit: 52,
        },
        ExecutableExampleCase {
            name: "async_main_aliased_guard_refined_const_backed_forwarded_nested_array_repackage_spawn",
            source_relative: "ramdon_tests/async_program_surface_examples/30_async_main_aliased_guard_refined_const_backed_forwarded_nested_array_repackage_spawn.ql",
            expected_exit: 54,
        },
        ExecutableExampleCase {
            name: "async_main_alias_sourced_composed_dynamic_forwarded_nested_array_repackage_spawn_with_tail_field",
            source_relative: "ramdon_tests/async_program_surface_examples/31_async_main_alias_sourced_composed_dynamic_forwarded_nested_array_repackage_spawn_with_tail_field.ql",
            expected_exit: 59,
        },
        ExecutableExampleCase {
            name: "async_main_guarded_alias_sourced_composed_dynamic_forwarded_nested_array_repackage_spawn_with_tail_field",
            source_relative: "ramdon_tests/async_program_surface_examples/32_async_main_guarded_alias_sourced_composed_dynamic_forwarded_nested_array_repackage_spawn_with_tail_field.ql",
            expected_exit: 63,
        },
        ExecutableExampleCase {
            name: "async_main_const_backed_alias_sourced_composed_dynamic_forwarded_nested_array_repackage_spawn",
            source_relative: "ramdon_tests/async_program_surface_examples/33_async_main_const_backed_alias_sourced_composed_dynamic_forwarded_nested_array_repackage_spawn.ql",
            expected_exit: 61,
        },
        ExecutableExampleCase {
            name: "async_main_guarded_const_backed_alias_sourced_composed_dynamic_forwarded_nested_array_repackage_spawn",
            source_relative: "ramdon_tests/async_program_surface_examples/34_async_main_guarded_const_backed_alias_sourced_composed_dynamic_forwarded_nested_array_repackage_spawn.ql",
            expected_exit: 62,
        },
        ExecutableExampleCase {
            name: "async_main_guarded_const_backed_double_root_alias_sourced_composed_dynamic_forwarded_nested_array_repackage_spawn",
            source_relative: "ramdon_tests/async_program_surface_examples/35_async_main_guarded_const_backed_double_root_alias_sourced_composed_dynamic_forwarded_nested_array_repackage_spawn.ql",
            expected_exit: 64,
        },
        ExecutableExampleCase {
            name: "async_main_guarded_const_backed_double_root_double_source_alias_sourced_composed_dynamic_forwarded_nested_array_repackage_spawn",
            source_relative: "ramdon_tests/async_program_surface_examples/36_async_main_guarded_const_backed_double_root_double_source_alias_sourced_composed_dynamic_forwarded_nested_array_repackage_spawn.ql",
            expected_exit: 66,
        },
        ExecutableExampleCase {
            name: "async_main_guarded_const_backed_double_root_double_source_row_alias_sourced_composed_dynamic_forwarded_nested_array_repackage_spawn",
            source_relative: "ramdon_tests/async_program_surface_examples/37_async_main_guarded_const_backed_double_root_double_source_row_alias_sourced_composed_dynamic_forwarded_nested_array_repackage_spawn.ql",
            expected_exit: 68,
        },
        ExecutableExampleCase {
            name: "async_main_guarded_const_backed_double_root_double_source_row_slot_alias_sourced_composed_dynamic_forwarded_nested_array_repackage_spawn",
            source_relative: "ramdon_tests/async_program_surface_examples/38_async_main_guarded_const_backed_double_root_double_source_row_slot_alias_sourced_composed_dynamic_forwarded_nested_array_repackage_spawn.ql",
            expected_exit: 72,
        },
        ExecutableExampleCase {
            name: "async_main_guarded_const_backed_triple_root_double_source_row_slot_alias_sourced_composed_dynamic_forwarded_nested_array_repackage_spawn",
            source_relative: "ramdon_tests/async_program_surface_examples/39_async_main_guarded_const_backed_triple_root_double_source_row_slot_alias_sourced_composed_dynamic_forwarded_nested_array_repackage_spawn.ql",
            expected_exit: 74,
        },
        ExecutableExampleCase {
            name: "async_main_guarded_const_backed_triple_root_triple_source_row_slot_alias_sourced_composed_dynamic_forwarded_nested_array_repackage_spawn",
            source_relative: "ramdon_tests/async_program_surface_examples/40_async_main_guarded_const_backed_triple_root_triple_source_row_slot_alias_sourced_composed_dynamic_forwarded_nested_array_repackage_spawn.ql",
            expected_exit: 76,
        },
        ExecutableExampleCase {
            name: "async_main_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_forwarded_nested_array_repackage_spawn",
            source_relative: "ramdon_tests/async_program_surface_examples/41_async_main_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_forwarded_nested_array_repackage_spawn.ql",
            expected_exit: 78,
        },
        ExecutableExampleCase {
            name: "async_main_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_forwarded_alias_nested_array_repackage_spawn",
            source_relative: "ramdon_tests/async_program_surface_examples/42_async_main_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_forwarded_alias_nested_array_repackage_spawn.ql",
            expected_exit: 80,
        },
        ExecutableExampleCase {
            name: "async_main_guarded_const_backed_triple_root_triple_source_tail_queued_spawn",
            source_relative: "ramdon_tests/async_program_surface_examples/43_async_main_guarded_const_backed_triple_root_triple_source_tail_queued_spawn.ql",
            expected_exit: 82,
        },
        ExecutableExampleCase {
            name: "async_main_guarded_const_backed_triple_root_triple_source_tail_queue_root_spawn",
            source_relative: "ramdon_tests/async_program_surface_examples/44_async_main_guarded_const_backed_triple_root_triple_source_tail_queue_root_spawn.ql",
            expected_exit: 84,
        },
        ExecutableExampleCase {
            name: "async_main_guarded_const_backed_triple_root_triple_source_tail_queue_root_alias_spawn",
            source_relative: "ramdon_tests/async_program_surface_examples/45_async_main_guarded_const_backed_triple_root_triple_source_tail_queue_root_alias_spawn.ql",
            expected_exit: 86,
        },
        ExecutableExampleCase {
            name: "async_main_guarded_const_backed_triple_root_triple_source_tail_queue_root_chain_spawn",
            source_relative: "ramdon_tests/async_program_surface_examples/46_async_main_guarded_const_backed_triple_root_triple_source_tail_queue_root_chain_spawn.ql",
            expected_exit: 88,
        },
        ExecutableExampleCase {
            name: "async_main_guarded_const_backed_triple_root_triple_source_tail_queue_local_alias_spawn",
            source_relative: "ramdon_tests/async_program_surface_examples/47_async_main_guarded_const_backed_triple_root_triple_source_tail_queue_local_alias_spawn.ql",
            expected_exit: 90,
        },
        ExecutableExampleCase {
            name: "async_main_guarded_const_backed_triple_root_triple_source_tail_queue_local_chain_spawn",
            source_relative: "ramdon_tests/async_program_surface_examples/48_async_main_guarded_const_backed_triple_root_triple_source_tail_queue_local_chain_spawn.ql",
            expected_exit: 92,
        },
        ExecutableExampleCase {
            name: "async_main_guarded_const_backed_triple_root_triple_source_tail_queue_local_forward_spawn",
            source_relative: "ramdon_tests/async_program_surface_examples/49_async_main_guarded_const_backed_triple_root_triple_source_tail_queue_local_forward_spawn.ql",
            expected_exit: 94,
        },
        ExecutableExampleCase {
            name: "async_main_guarded_const_backed_triple_root_triple_source_tail_queue_local_inline_forward_spawn",
            source_relative: "ramdon_tests/async_program_surface_examples/50_async_main_guarded_const_backed_triple_root_triple_source_tail_queue_local_inline_forward_spawn.ql",
            expected_exit: 96,
        },
        ExecutableExampleCase {
            name: "async_main_guarded_const_backed_triple_root_triple_source_tail_bundle_inline_forward_spawn",
            source_relative: "ramdon_tests/async_program_surface_examples/51_async_main_guarded_const_backed_triple_root_triple_source_tail_bundle_inline_forward_spawn.ql",
            expected_exit: 98,
        },
        ExecutableExampleCase {
            name: "async_main_guarded_const_backed_triple_root_triple_source_tail_bundle_slot_inline_forward_spawn",
            source_relative: "ramdon_tests/async_program_surface_examples/52_async_main_guarded_const_backed_triple_root_triple_source_tail_bundle_slot_inline_forward_spawn.ql",
            expected_exit: 100,
        },
        ExecutableExampleCase {
            name: "async_main_guarded_const_backed_triple_root_triple_source_tail_direct_inline_forward_spawn",
            source_relative: "ramdon_tests/async_program_surface_examples/53_async_main_guarded_const_backed_triple_root_triple_source_tail_direct_inline_forward_spawn.ql",
            expected_exit: 102,
        },
        ExecutableExampleCase {
            name: "async_main_guarded_const_backed_triple_root_triple_source_tail_direct_inline_forward_await",
            source_relative: "ramdon_tests/async_program_surface_examples/54_async_main_guarded_const_backed_triple_root_triple_source_tail_direct_inline_forward_await.ql",
            expected_exit: 104,
        },
        ExecutableExampleCase {
            name: "async_main_guarded_const_backed_triple_root_triple_source_tail_bundle_slot_inline_forward_await",
            source_relative: "ramdon_tests/async_program_surface_examples/55_async_main_guarded_const_backed_triple_root_triple_source_tail_bundle_slot_inline_forward_await.ql",
            expected_exit: 106,
        },
        ExecutableExampleCase {
            name: "async_main_guarded_const_backed_triple_root_triple_source_tail_bundle_inline_forward_await",
            source_relative: "ramdon_tests/async_program_surface_examples/56_async_main_guarded_const_backed_triple_root_triple_source_tail_bundle_inline_forward_await.ql",
            expected_exit: 108,
        },
        ExecutableExampleCase {
            name: "async_main_guarded_const_backed_triple_root_triple_source_tail_queue_local_inline_forward_await",
            source_relative: "ramdon_tests/async_program_surface_examples/57_async_main_guarded_const_backed_triple_root_triple_source_tail_queue_local_inline_forward_await.ql",
            expected_exit: 110,
        },
        ExecutableExampleCase {
            name: "async_main_guarded_const_backed_triple_root_triple_source_tail_queue_local_forward_await",
            source_relative: "ramdon_tests/async_program_surface_examples/58_async_main_guarded_const_backed_triple_root_triple_source_tail_queue_local_forward_await.ql",
            expected_exit: 112,
        },
        ExecutableExampleCase {
            name: "async_main_guarded_const_backed_triple_root_triple_source_tail_queue_root_inline_forward_await",
            source_relative: "ramdon_tests/async_program_surface_examples/59_async_main_guarded_const_backed_triple_root_triple_source_tail_queue_root_inline_forward_await.ql",
            expected_exit: 114,
        },
        ExecutableExampleCase {
            name: "async_main_guarded_const_backed_triple_root_triple_source_tail_queue_root_forward_await",
            source_relative: "ramdon_tests/async_program_surface_examples/60_async_main_guarded_const_backed_triple_root_triple_source_tail_queue_root_forward_await.ql",
            expected_exit: 116,
        },
        ExecutableExampleCase {
            name: "async_main_guarded_const_backed_triple_root_triple_source_tail_queue_root_alias_forward_await",
            source_relative: "ramdon_tests/async_program_surface_examples/61_async_main_guarded_const_backed_triple_root_triple_source_tail_queue_root_alias_forward_await.ql",
            expected_exit: 118,
        },
        ExecutableExampleCase {
            name: "async_main_guarded_const_backed_triple_root_triple_source_tail_queue_root_chain_forward_await",
            source_relative: "ramdon_tests/async_program_surface_examples/62_async_main_guarded_const_backed_triple_root_triple_source_tail_queue_root_chain_forward_await.ql",
            expected_exit: 120,
        },
        ExecutableExampleCase {
            name: "async_main_guarded_const_backed_triple_root_triple_source_tail_queue_root_alias_inline_forward_await",
            source_relative: "ramdon_tests/async_program_surface_examples/63_async_main_guarded_const_backed_triple_root_triple_source_tail_queue_root_alias_inline_forward_await.ql",
            expected_exit: 122,
        },
        ExecutableExampleCase {
            name: "async_main_guarded_const_backed_triple_root_triple_source_tail_queue_root_chain_inline_forward_await",
            source_relative: "ramdon_tests/async_program_surface_examples/64_async_main_guarded_const_backed_triple_root_triple_source_tail_queue_root_chain_inline_forward_await.ql",
            expected_exit: 124,
        },
        ExecutableExampleCase {
            name: "async_main_guarded_const_backed_triple_root_triple_source_tail_bundle_forward_await",
            source_relative: "ramdon_tests/async_program_surface_examples/65_async_main_guarded_const_backed_triple_root_triple_source_tail_bundle_forward_await.ql",
            expected_exit: 126,
        },
        ExecutableExampleCase {
            name: "async_main_guarded_const_backed_triple_root_triple_source_tail_bundle_alias_forward_await",
            source_relative: "ramdon_tests/async_program_surface_examples/66_async_main_guarded_const_backed_triple_root_triple_source_tail_bundle_alias_forward_await.ql",
            expected_exit: 128,
        },
        ExecutableExampleCase {
            name: "async_main_guarded_const_backed_triple_root_triple_source_tail_bundle_chain_forward_await",
            source_relative: "ramdon_tests/async_program_surface_examples/67_async_main_guarded_const_backed_triple_root_triple_source_tail_bundle_chain_forward_await.ql",
            expected_exit: 130,
        },
        ExecutableExampleCase {
            name: "async_main_guarded_const_backed_triple_root_triple_source_tail_bundle_alias_inline_forward_await",
            source_relative: "ramdon_tests/async_program_surface_examples/68_async_main_guarded_const_backed_triple_root_triple_source_tail_bundle_alias_inline_forward_await.ql",
            expected_exit: 132,
        },
        ExecutableExampleCase {
            name: "async_main_guarded_const_backed_triple_root_triple_source_tail_bundle_chain_inline_forward_await",
            source_relative: "ramdon_tests/async_program_surface_examples/69_async_main_guarded_const_backed_triple_root_triple_source_tail_bundle_chain_inline_forward_await.ql",
            expected_exit: 134,
        },
    ];

    assert_example_cases_run(
        &workspace_root,
        &cases,
        "async executable example regressions",
    );
}

fn toolchain_available(context: &str) -> bool {
    let Ok(_toolchain) = discover_toolchain(&ToolchainOptions::default()) else {
        eprintln!(
            "skipping {context}: no clang-style compiler found via ql-driver toolchain discovery"
        );
        return false;
    };
    true
}

fn assert_example_cases_run(
    workspace_root: &Path,
    cases: &[ExecutableExampleCase],
    failure_header: &str,
) {
    let mut failures = Vec::new();
    for case in cases {
        if let Err(message) = run_executable_example_case(workspace_root, case) {
            failures.push(message);
        }
    }

    assert!(
        failures.is_empty(),
        "{failure_header}:\n\n{}",
        failures.join("\n\n")
    );
}

#[derive(Clone, Copy)]
struct ExecutableExampleCase {
    name: &'static str,
    source_relative: &'static str,
    expected_exit: i32,
}

fn run_executable_example_case(
    workspace_root: &Path,
    case: &ExecutableExampleCase,
) -> Result<(), String> {
    let temp = TempDir::new(&format!("ql-executable-example-{}", case.name));
    let output_path = executable_output_path(temp.path(), "artifact");
    let build_output = run_ql_build_capture(
        workspace_root,
        case.source_relative,
        "exe",
        &output_path,
        &[],
    );
    let (build_stdout, build_stderr) = expect_success(case.name, "build", &build_output)?;

    let expected_build_stdout = format!("wrote executable: {}", output_path.display());
    if build_stdout.trim() != expected_build_stdout || !build_stderr.trim().is_empty() {
        return Err(format!(
            "[{}] unexpected successful build output\n--- expected stdout ---\n{}\n--- actual stdout ---\n{}\n--- stderr ---\n{}",
            case.name, expected_build_stdout, build_stdout, build_stderr
        ));
    }

    expect_file_exists(case.name, &output_path, "built executable", "build")?;

    let mut run_command = Command::new(&output_path);
    run_command.current_dir(workspace_root);
    let run_output = run_command_capture(
        &mut run_command,
        format!("built executable `{}`", output_path.display()),
    );
    let (run_stdout, run_stderr) = expect_exit_code(
        case.name,
        "runtime executable",
        &run_output,
        case.expected_exit,
    )?;
    expect_silent_output(case.name, "executable run", &run_stdout, &run_stderr)?;

    Ok(())
}
