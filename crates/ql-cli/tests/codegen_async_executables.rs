mod support;

use support::codegen::{PassCase, run_pass_case};
use support::workspace_root;

#[test]
fn async_executable_codegen_cases_match() {
    let workspace_root = workspace_root();
    let mut failures = Vec::new();

    for stem in async_executable_stems() {
        let source_relative = format!("fixtures/codegen/pass/{stem}.ql");
        let case_name = format!("{stem}_exe");
        let pass_case = PassCase {
            name: &case_name,
            source_relative: &source_relative,
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        };

        if let Err(message) = run_pass_case(&workspace_root, &pass_case) {
            failures.push(format!("{} executable regression:\n\n{}", stem, message));
        }
    }

    assert!(
        failures.is_empty(),
        "async executable codegen regressions:\n\n{}",
        failures.join("\n\n")
    );
}

fn async_executable_stems() -> &'static [&'static str] {
    &[
        "async_program_main",
        "async_program_main_for_await_array",
        "async_program_main_nested_task_handle",
        "async_program_main_task_handle_tuple_payload",
        "async_program_main_task_handle_array_payload",
        "async_program_main_nested_aggregate_task_handle_payload",
        "async_program_main_helper_task_handle_flows",
        "async_program_main_zero_sized_helper_task_handle_flows",
        "async_program_main_local_return_task_handle",
        "async_program_main_direct_handle",
        "async_program_main_spawn_bound_task_handle",
        "async_program_main_local_return_zero_sized_task_handle",
        "async_program_main_zero_sized_aggregate_results",
        "async_program_main_spawn_zero_sized_aggregate_result",
        "async_program_main_aggregate_results",
        "async_program_main_spawned_aggregate_results",
        "async_program_main_recursive_aggregate_results",
        "async_program_main_spawned_recursive_aggregate_results",
        "async_program_main_recursive_aggregate_params",
        "async_program_main_spawned_recursive_aggregate_params",
        "async_program_main_zero_sized_aggregate_params",
        "async_program_main_spawned_zero_sized_aggregate_params",
        "async_program_main_projected_task_handle_awaits",
        "async_program_main_projected_task_handle_spawns",
        "async_program_main_projected_task_handle_reinit",
        "async_program_main_projected_task_handle_conditional_reinit",
        "async_program_main_zero_sized_nested_task_handle",
        "async_program_main_zero_sized_struct_task_handle_payload",
        "async_program_main_zero_sized_projected_task_handle_awaits",
        "async_program_main_zero_sized_projected_task_handle_spawns",
        "async_program_main_zero_sized_projected_task_handle_reinit",
        "async_program_main_zero_sized_projected_task_handle_conditional_reinit",
        "async_program_main_branch_spawned_reinit",
        "async_program_main_zero_sized_branch_spawned_reinit",
        "async_program_main_zero_sized_reverse_branch_spawned_reinit",
        "async_program_main_conditional_async_call_spawns",
        "async_program_main_zero_sized_conditional_async_call_spawns",
        "async_program_main_conditional_helper_task_handle_spawns",
        "async_program_main_zero_sized_conditional_helper_task_handle_spawns",
    ]
}
