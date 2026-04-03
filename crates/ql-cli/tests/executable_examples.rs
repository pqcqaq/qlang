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
        ExecutableExampleCase {
            name: "sync_static_item_values",
            source_relative: "ramdon_tests/executable_examples/04_sync_static_item_values.ql",
            expected_exit: 5,
        },
        ExecutableExampleCase {
            name: "sync_named_call_arguments",
            source_relative: "ramdon_tests/executable_examples/05_sync_named_call_arguments.ql",
            expected_exit: 47,
        },
        ExecutableExampleCase {
            name: "sync_import_alias_named_call_arguments",
            source_relative: "ramdon_tests/executable_examples/06_sync_import_alias_named_call_arguments.ql",
            expected_exit: 49,
        },
        ExecutableExampleCase {
            name: "sync_for_fixed_array",
            source_relative: "ramdon_tests/executable_examples/07_sync_for_fixed_array.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "sync_for_tuple",
            source_relative: "ramdon_tests/executable_examples/08_sync_for_tuple.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "sync_for_projected_fixed_shape",
            source_relative: "ramdon_tests/executable_examples/09_sync_for_projected_fixed_shape.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "sync_for_const_static_fixed_shape",
            source_relative: "ramdon_tests/executable_examples/10_sync_for_const_static_fixed_shape.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "sync_match_scrutinee_self_guard",
            source_relative: "ramdon_tests/executable_examples/11_sync_match_scrutinee_self_guard.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "sync_match_scrutinee_bool_comparison_guard",
            source_relative: "ramdon_tests/executable_examples/12_sync_match_scrutinee_bool_comparison_guard.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "sync_match_partial_dynamic_guard",
            source_relative: "ramdon_tests/executable_examples/13_sync_match_partial_dynamic_guard.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "sync_match_partial_integer_dynamic_guard",
            source_relative: "ramdon_tests/executable_examples/14_sync_match_partial_integer_dynamic_guard.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "sync_match_guard_binding_projection_roots",
            source_relative: "ramdon_tests/executable_examples/15_sync_match_guard_binding_projection_roots.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "sync_match_binding_catch_all_aggregate_scrutinees",
            source_relative: "ramdon_tests/executable_examples/16_sync_match_binding_catch_all_aggregate_scrutinees.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "sync_match_guard_runtime_index_item_roots",
            source_relative: "ramdon_tests/executable_examples/17_sync_match_guard_runtime_index_item_roots.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "sync_match_guard_direct_calls",
            source_relative: "ramdon_tests/executable_examples/18_sync_match_guard_direct_calls.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "sync_match_guard_call_projection_roots",
            source_relative: "ramdon_tests/executable_examples/19_sync_match_guard_call_projection_roots.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "sync_match_guard_aggregate_call_args",
            source_relative: "ramdon_tests/executable_examples/20_sync_match_guard_aggregate_call_args.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "sync_match_guard_inline_aggregate_call_args",
            source_relative: "ramdon_tests/executable_examples/21_sync_match_guard_inline_aggregate_call_args.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "sync_match_guard_inline_projection_roots",
            source_relative: "ramdon_tests/executable_examples/22_sync_match_guard_inline_projection_roots.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "sync_match_guard_item_backed_inline_combos",
            source_relative: "ramdon_tests/executable_examples/23_sync_match_guard_item_backed_inline_combos.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "sync_match_guard_call_backed_combos",
            source_relative: "ramdon_tests/executable_examples/24_sync_match_guard_call_backed_combos.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "sync_match_guard_call_root_nested_runtime_projection",
            source_relative: "ramdon_tests/executable_examples/25_sync_match_guard_call_root_nested_runtime_projection.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "sync_match_guard_nested_call_root_inline_combos",
            source_relative: "ramdon_tests/executable_examples/26_sync_match_guard_nested_call_root_inline_combos.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "sync_match_guard_item_backed_nested_call_root_combos",
            source_relative: "ramdon_tests/executable_examples/27_sync_match_guard_item_backed_nested_call_root_combos.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "sync_match_guard_call_backed_nested_call_root_combos",
            source_relative: "ramdon_tests/executable_examples/28_sync_match_guard_call_backed_nested_call_root_combos.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "sync_match_guard_alias_backed_nested_call_root_combos",
            source_relative: "ramdon_tests/executable_examples/29_sync_match_guard_alias_backed_nested_call_root_combos.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "sync_match_guard_binding_backed_nested_call_root_combos",
            source_relative: "ramdon_tests/executable_examples/30_sync_match_guard_binding_backed_nested_call_root_combos.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "sync_match_guard_projection_backed_nested_call_root_combos",
            source_relative: "ramdon_tests/executable_examples/31_sync_match_guard_projection_backed_nested_call_root_combos.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "sync_for_call_root_fixed_shapes",
            source_relative: "ramdon_tests/executable_examples/32_sync_for_call_root_fixed_shapes.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "sync_import_alias_call_root_fixed_shapes",
            source_relative: "ramdon_tests/executable_examples/33_sync_import_alias_call_root_fixed_shapes.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "sync_nested_call_root_fixed_shapes",
            source_relative: "ramdon_tests/executable_examples/34_sync_nested_call_root_fixed_shapes.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "sync_import_alias_nested_call_root_fixed_shapes",
            source_relative: "ramdon_tests/executable_examples/35_sync_import_alias_nested_call_root_fixed_shapes.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "sync_inline_projected_fixed_shapes",
            source_relative: "ramdon_tests/executable_examples/36_sync_inline_projected_fixed_shapes.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "sync_import_alias_inline_projected_fixed_shapes",
            source_relative: "ramdon_tests/executable_examples/37_sync_import_alias_inline_projected_fixed_shapes.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "sync_inline_projected_fixed_shapes_without_parens",
            source_relative: "ramdon_tests/executable_examples/38_sync_inline_projected_fixed_shapes_without_parens.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "sync_unparenthesized_inline_projected_control_flow_heads",
            source_relative: "ramdon_tests/executable_examples/39_sync_unparenthesized_inline_projected_control_flow_heads.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "sync_unsafe_function_bodies",
            source_relative: "ramdon_tests/executable_examples/40_sync_unsafe_function_bodies.ql",
            expected_exit: 5,
        },
        ExecutableExampleCase {
            name: "sync_assignment_expressions",
            source_relative: "ramdon_tests/executable_examples/41_sync_assignment_expressions.ql",
            expected_exit: 38,
        },
        ExecutableExampleCase {
            name: "sync_dynamic_array_assignments",
            source_relative: "ramdon_tests/executable_examples/42_sync_dynamic_array_assignments.ql",
            expected_exit: 18,
        },
        ExecutableExampleCase {
            name: "sync_tuple_assignment_expressions",
            source_relative: "ramdon_tests/executable_examples/43_sync_tuple_assignment_expressions.ql",
            expected_exit: 19,
        },
        ExecutableExampleCase {
            name: "sync_projected_root_dynamic_array_assignments",
            source_relative: "ramdon_tests/executable_examples/44_sync_projected_root_dynamic_array_assignments.ql",
            expected_exit: 8,
        },
        ExecutableExampleCase {
            name: "sync_projected_root_tuple_assignment_expressions",
            source_relative: "ramdon_tests/executable_examples/45_sync_projected_root_tuple_assignment_expressions.ql",
            expected_exit: 19,
        },
        ExecutableExampleCase {
            name: "sync_projected_root_assignment_expressions",
            source_relative: "ramdon_tests/executable_examples/46_sync_projected_root_assignment_expressions.ql",
            expected_exit: 13,
        },
        ExecutableExampleCase {
            name: "sync_dynamic_assignment_expressions",
            source_relative: "ramdon_tests/executable_examples/47_sync_dynamic_assignment_expressions.ql",
            expected_exit: 38,
        },
        ExecutableExampleCase {
            name: "sync_nested_projected_dynamic_assignment_expressions",
            source_relative: "ramdon_tests/executable_examples/48_sync_nested_projected_dynamic_assignment_expressions.ql",
            expected_exit: 16,
        },
        ExecutableExampleCase {
            name: "sync_nested_projected_tuple_assignment_expressions",
            source_relative: "ramdon_tests/executable_examples/49_sync_nested_projected_tuple_assignment_expressions.ql",
            expected_exit: 20,
        },
        ExecutableExampleCase {
            name: "sync_nested_projected_assignment_expressions",
            source_relative: "ramdon_tests/executable_examples/50_sync_nested_projected_assignment_expressions.ql",
            expected_exit: 14,
        },
        ExecutableExampleCase {
            name: "sync_call_root_nested_projected_assignment_expressions",
            source_relative: "ramdon_tests/executable_examples/51_sync_call_root_nested_projected_assignment_expressions.ql",
            expected_exit: 14,
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
        ExecutableExampleCase {
            name: "async_main_guarded_const_backed_triple_root_triple_source_tail_bundle_forward_spawn",
            source_relative: "ramdon_tests/async_program_surface_examples/70_async_main_guarded_const_backed_triple_root_triple_source_tail_bundle_forward_spawn.ql",
            expected_exit: 136,
        },
        ExecutableExampleCase {
            name: "async_main_guarded_const_backed_triple_root_triple_source_tail_bundle_alias_forward_spawn",
            source_relative: "ramdon_tests/async_program_surface_examples/71_async_main_guarded_const_backed_triple_root_triple_source_tail_bundle_alias_forward_spawn.ql",
            expected_exit: 138,
        },
        ExecutableExampleCase {
            name: "async_main_guarded_const_backed_triple_root_triple_source_tail_bundle_chain_forward_spawn",
            source_relative: "ramdon_tests/async_program_surface_examples/72_async_main_guarded_const_backed_triple_root_triple_source_tail_bundle_chain_forward_spawn.ql",
            expected_exit: 140,
        },
        ExecutableExampleCase {
            name: "async_main_aliased_guard_refined_static_alias_backed_projected_root",
            source_relative: "ramdon_tests/async_program_surface_examples/73_async_main_aliased_guard_refined_static_alias_backed_projected_root.ql",
            expected_exit: 35,
        },
        ExecutableExampleCase {
            name: "async_main_aliased_guard_refined_static_alias_backed_nested_repackage_spawn",
            source_relative: "ramdon_tests/async_program_surface_examples/74_async_main_aliased_guard_refined_static_alias_backed_nested_repackage_spawn.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "async_main_aliased_guard_refined_static_alias_backed_forwarded_nested_array_repackage_spawn",
            source_relative: "ramdon_tests/async_program_surface_examples/75_async_main_aliased_guard_refined_static_alias_backed_forwarded_nested_array_repackage_spawn.ql",
            expected_exit: 58,
        },
        ExecutableExampleCase {
            name: "async_main_guarded_static_alias_sourced_composed_dynamic_forwarded_nested_array_repackage_spawn",
            source_relative: "ramdon_tests/async_program_surface_examples/76_async_main_guarded_static_alias_sourced_composed_dynamic_forwarded_nested_array_repackage_spawn.ql",
            expected_exit: 65,
        },
        ExecutableExampleCase {
            name: "async_main_guarded_static_alias_backed_double_root_alias_sourced_composed_dynamic_forwarded_nested_array_repackage_spawn",
            source_relative: "ramdon_tests/async_program_surface_examples/77_async_main_guarded_static_alias_backed_double_root_alias_sourced_composed_dynamic_forwarded_nested_array_repackage_spawn.ql",
            expected_exit: 69,
        },
        ExecutableExampleCase {
            name: "async_main_guarded_static_alias_backed_double_root_double_source_alias_sourced_composed_dynamic_forwarded_nested_array_repackage_spawn",
            source_relative: "ramdon_tests/async_program_surface_examples/78_async_main_guarded_static_alias_backed_double_root_double_source_alias_sourced_composed_dynamic_forwarded_nested_array_repackage_spawn.ql",
            expected_exit: 71,
        },
        ExecutableExampleCase {
            name: "async_main_guarded_static_alias_backed_double_root_double_source_row_alias_sourced_composed_dynamic_forwarded_nested_array_repackage_spawn",
            source_relative: "ramdon_tests/async_program_surface_examples/79_async_main_guarded_static_alias_backed_double_root_double_source_row_alias_sourced_composed_dynamic_forwarded_nested_array_repackage_spawn.ql",
            expected_exit: 73,
        },
        ExecutableExampleCase {
            name: "async_main_guarded_static_alias_backed_double_root_double_source_row_slot_alias_sourced_composed_dynamic_forwarded_nested_array_repackage_spawn",
            source_relative: "ramdon_tests/async_program_surface_examples/80_async_main_guarded_static_alias_backed_double_root_double_source_row_slot_alias_sourced_composed_dynamic_forwarded_nested_array_repackage_spawn.ql",
            expected_exit: 77,
        },
        ExecutableExampleCase {
            name: "async_main_guarded_static_alias_backed_triple_root_double_source_row_slot_alias_sourced_composed_dynamic_forwarded_nested_array_repackage_spawn",
            source_relative: "ramdon_tests/async_program_surface_examples/81_async_main_guarded_static_alias_backed_triple_root_double_source_row_slot_alias_sourced_composed_dynamic_forwarded_nested_array_repackage_spawn.ql",
            expected_exit: 79,
        },
        ExecutableExampleCase {
            name: "async_main_guarded_static_alias_backed_triple_root_triple_source_row_slot_alias_sourced_composed_dynamic_forwarded_nested_array_repackage_spawn",
            source_relative: "ramdon_tests/async_program_surface_examples/82_async_main_guarded_static_alias_backed_triple_root_triple_source_row_slot_alias_sourced_composed_dynamic_forwarded_nested_array_repackage_spawn.ql",
            expected_exit: 81,
        },
        ExecutableExampleCase {
            name: "async_main_guarded_static_alias_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_forwarded_nested_array_repackage_spawn",
            source_relative: "ramdon_tests/async_program_surface_examples/83_async_main_guarded_static_alias_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_forwarded_nested_array_repackage_spawn.ql",
            expected_exit: 84,
        },
        ExecutableExampleCase {
            name: "async_main_guarded_static_alias_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_forwarded_alias_nested_array_repackage_spawn",
            source_relative: "ramdon_tests/async_program_surface_examples/84_async_main_guarded_static_alias_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_forwarded_alias_nested_array_repackage_spawn.ql",
            expected_exit: 86,
        },
        ExecutableExampleCase {
            name: "async_main_guarded_static_alias_backed_triple_root_triple_source_tail_queued_spawn",
            source_relative: "ramdon_tests/async_program_surface_examples/85_async_main_guarded_static_alias_backed_triple_root_triple_source_tail_queued_spawn.ql",
            expected_exit: 88,
        },
        ExecutableExampleCase {
            name: "async_main_guarded_static_alias_backed_triple_root_triple_source_tail_queue_root_spawn",
            source_relative: "ramdon_tests/async_program_surface_examples/86_async_main_guarded_static_alias_backed_triple_root_triple_source_tail_queue_root_spawn.ql",
            expected_exit: 90,
        },
        ExecutableExampleCase {
            name: "async_main_guarded_static_alias_backed_triple_root_triple_source_tail_queue_root_alias_spawn",
            source_relative: "ramdon_tests/async_program_surface_examples/87_async_main_guarded_static_alias_backed_triple_root_triple_source_tail_queue_root_alias_spawn.ql",
            expected_exit: 92,
        },
        ExecutableExampleCase {
            name: "async_main_guarded_static_alias_backed_triple_root_triple_source_tail_queue_root_chain_spawn",
            source_relative: "ramdon_tests/async_program_surface_examples/88_async_main_guarded_static_alias_backed_triple_root_triple_source_tail_queue_root_chain_spawn.ql",
            expected_exit: 94,
        },
        ExecutableExampleCase {
            name: "async_main_guarded_static_alias_backed_triple_root_triple_source_tail_queue_local_alias_spawn",
            source_relative: "ramdon_tests/async_program_surface_examples/89_async_main_guarded_static_alias_backed_triple_root_triple_source_tail_queue_local_alias_spawn.ql",
            expected_exit: 96,
        },
        ExecutableExampleCase {
            name: "async_main_guarded_static_alias_backed_triple_root_triple_source_tail_queue_local_chain_spawn",
            source_relative: "ramdon_tests/async_program_surface_examples/90_async_main_guarded_static_alias_backed_triple_root_triple_source_tail_queue_local_chain_spawn.ql",
            expected_exit: 98,
        },
        ExecutableExampleCase {
            name: "async_main_guarded_static_alias_backed_triple_root_triple_source_tail_queue_local_forward_spawn",
            source_relative: "ramdon_tests/async_program_surface_examples/91_async_main_guarded_static_alias_backed_triple_root_triple_source_tail_queue_local_forward_spawn.ql",
            expected_exit: 100,
        },
        ExecutableExampleCase {
            name: "async_main_guarded_static_alias_backed_triple_root_triple_source_tail_queue_local_inline_forward_spawn",
            source_relative: "ramdon_tests/async_program_surface_examples/92_async_main_guarded_static_alias_backed_triple_root_triple_source_tail_queue_local_inline_forward_spawn.ql",
            expected_exit: 102,
        },
        ExecutableExampleCase {
            name: "async_main_guarded_static_alias_backed_triple_root_triple_source_tail_bundle_inline_forward_spawn",
            source_relative: "ramdon_tests/async_program_surface_examples/93_async_main_guarded_static_alias_backed_triple_root_triple_source_tail_bundle_inline_forward_spawn.ql",
            expected_exit: 104,
        },
        ExecutableExampleCase {
            name: "async_main_guarded_static_alias_backed_triple_root_triple_source_tail_bundle_slot_inline_forward_spawn",
            source_relative: "ramdon_tests/async_program_surface_examples/94_async_main_guarded_static_alias_backed_triple_root_triple_source_tail_bundle_slot_inline_forward_spawn.ql",
            expected_exit: 106,
        },
        ExecutableExampleCase {
            name: "async_main_guarded_static_alias_backed_triple_root_triple_source_tail_direct_inline_forward_spawn",
            source_relative: "ramdon_tests/async_program_surface_examples/95_async_main_guarded_static_alias_backed_triple_root_triple_source_tail_direct_inline_forward_spawn.ql",
            expected_exit: 108,
        },
        ExecutableExampleCase {
            name: "async_main_guarded_static_alias_backed_triple_root_triple_source_tail_direct_inline_forward_await",
            source_relative: "ramdon_tests/async_program_surface_examples/96_async_main_guarded_static_alias_backed_triple_root_triple_source_tail_direct_inline_forward_await.ql",
            expected_exit: 110,
        },
        ExecutableExampleCase {
            name: "async_main_guarded_static_alias_backed_triple_root_triple_source_tail_bundle_slot_inline_forward_await",
            source_relative: "ramdon_tests/async_program_surface_examples/97_async_main_guarded_static_alias_backed_triple_root_triple_source_tail_bundle_slot_inline_forward_await.ql",
            expected_exit: 112,
        },
        ExecutableExampleCase {
            name: "async_main_guarded_static_alias_backed_triple_root_triple_source_tail_bundle_inline_forward_await",
            source_relative: "ramdon_tests/async_program_surface_examples/98_async_main_guarded_static_alias_backed_triple_root_triple_source_tail_bundle_inline_forward_await.ql",
            expected_exit: 114,
        },
        ExecutableExampleCase {
            name: "async_main_guarded_static_alias_backed_triple_root_triple_source_tail_queue_local_inline_forward_await",
            source_relative: "ramdon_tests/async_program_surface_examples/99_async_main_guarded_static_alias_backed_triple_root_triple_source_tail_queue_local_inline_forward_await.ql",
            expected_exit: 116,
        },
        ExecutableExampleCase {
            name: "async_main_guarded_static_alias_backed_triple_root_triple_source_tail_queue_local_forward_await",
            source_relative: "ramdon_tests/async_program_surface_examples/100_async_main_guarded_static_alias_backed_triple_root_triple_source_tail_queue_local_forward_await.ql",
            expected_exit: 118,
        },
        ExecutableExampleCase {
            name: "async_main_guarded_static_alias_backed_triple_root_triple_source_tail_queue_root_inline_forward_await",
            source_relative: "ramdon_tests/async_program_surface_examples/101_async_main_guarded_static_alias_backed_triple_root_triple_source_tail_queue_root_inline_forward_await.ql",
            expected_exit: 120,
        },
        ExecutableExampleCase {
            name: "async_main_guarded_static_alias_backed_triple_root_triple_source_tail_queue_root_forward_await",
            source_relative: "ramdon_tests/async_program_surface_examples/102_async_main_guarded_static_alias_backed_triple_root_triple_source_tail_queue_root_forward_await.ql",
            expected_exit: 122,
        },
        ExecutableExampleCase {
            name: "async_main_guarded_static_alias_backed_triple_root_triple_source_tail_queue_root_alias_forward_await",
            source_relative: "ramdon_tests/async_program_surface_examples/103_async_main_guarded_static_alias_backed_triple_root_triple_source_tail_queue_root_alias_forward_await.ql",
            expected_exit: 124,
        },
        ExecutableExampleCase {
            name: "async_main_guarded_static_alias_backed_triple_root_triple_source_tail_queue_root_chain_forward_await",
            source_relative: "ramdon_tests/async_program_surface_examples/104_async_main_guarded_static_alias_backed_triple_root_triple_source_tail_queue_root_chain_forward_await.ql",
            expected_exit: 126,
        },
        ExecutableExampleCase {
            name: "async_main_guarded_static_alias_backed_triple_root_triple_source_tail_queue_root_alias_inline_forward_await",
            source_relative: "ramdon_tests/async_program_surface_examples/105_async_main_guarded_static_alias_backed_triple_root_triple_source_tail_queue_root_alias_inline_forward_await.ql",
            expected_exit: 128,
        },
        ExecutableExampleCase {
            name: "async_main_guarded_static_alias_backed_triple_root_triple_source_tail_queue_root_chain_inline_forward_await",
            source_relative: "ramdon_tests/async_program_surface_examples/106_async_main_guarded_static_alias_backed_triple_root_triple_source_tail_queue_root_chain_inline_forward_await.ql",
            expected_exit: 130,
        },
        ExecutableExampleCase {
            name: "async_main_import_alias_named_calls",
            source_relative: "ramdon_tests/async_program_surface_examples/107_async_main_import_alias_named_calls.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "async_main_import_alias_direct_submit",
            source_relative: "ramdon_tests/async_program_surface_examples/108_async_main_import_alias_direct_submit.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "async_main_import_alias_aggregate_submit",
            source_relative: "ramdon_tests/async_program_surface_examples/109_async_main_import_alias_aggregate_submit.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "async_main_import_alias_array_submit",
            source_relative: "ramdon_tests/async_program_surface_examples/110_async_main_import_alias_array_submit.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "async_main_import_alias_tuple_submit",
            source_relative: "ramdon_tests/async_program_surface_examples/111_async_main_import_alias_tuple_submit.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "async_main_import_alias_forward_submit",
            source_relative: "ramdon_tests/async_program_surface_examples/112_async_main_import_alias_forward_submit.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "async_main_import_alias_helper_task_submit",
            source_relative: "ramdon_tests/async_program_surface_examples/113_async_main_import_alias_helper_task_submit.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "async_main_import_alias_helper_forward_submit",
            source_relative: "ramdon_tests/async_program_surface_examples/114_async_main_import_alias_helper_forward_submit.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "async_main_import_alias_task_array_for_await",
            source_relative: "ramdon_tests/async_program_surface_examples/115_async_main_import_alias_task_array_for_await.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "async_main_import_alias_helper_task_array_for_await",
            source_relative: "ramdon_tests/async_program_surface_examples/116_async_main_import_alias_helper_task_array_for_await.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "async_main_projected_task_array_for_await",
            source_relative: "ramdon_tests/async_program_surface_examples/117_async_main_projected_task_array_for_await.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "async_main_void_task_array_for_await",
            source_relative: "ramdon_tests/async_program_surface_examples/118_async_main_void_task_array_for_await.ql",
            expected_exit: 2,
        },
        ExecutableExampleCase {
            name: "async_main_awaited_aggregate_task_array_for_await",
            source_relative: "ramdon_tests/async_program_surface_examples/119_async_main_awaited_aggregate_task_array_for_await.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "async_main_awaited_nested_aggregate_task_array_for_await",
            source_relative: "ramdon_tests/async_program_surface_examples/120_async_main_awaited_nested_aggregate_task_array_for_await.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "async_main_import_alias_awaited_aggregate_task_array_for_await",
            source_relative: "ramdon_tests/async_program_surface_examples/121_async_main_import_alias_awaited_aggregate_task_array_for_await.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "async_main_inline_awaited_task_array_for_await",
            source_relative: "ramdon_tests/async_program_surface_examples/122_async_main_inline_awaited_task_array_for_await.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "async_main_awaited_tuple_projected_task_array_for_await",
            source_relative: "ramdon_tests/async_program_surface_examples/123_async_main_awaited_tuple_projected_task_array_for_await.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "async_main_awaited_array_projected_task_array_for_await",
            source_relative: "ramdon_tests/async_program_surface_examples/124_async_main_awaited_array_projected_task_array_for_await.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "async_main_tuple_for_await",
            source_relative: "ramdon_tests/async_program_surface_examples/125_async_main_tuple_for_await.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "async_main_task_tuple_for_await",
            source_relative: "ramdon_tests/async_program_surface_examples/126_async_main_task_tuple_for_await.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "async_main_import_alias_helper_task_tuple_for_await",
            source_relative: "ramdon_tests/async_program_surface_examples/127_async_main_import_alias_helper_task_tuple_for_await.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "async_main_projected_task_tuple_for_await",
            source_relative: "ramdon_tests/async_program_surface_examples/128_async_main_projected_task_tuple_for_await.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "async_main_awaited_aggregate_task_tuple_for_await",
            source_relative: "ramdon_tests/async_program_surface_examples/129_async_main_awaited_aggregate_task_tuple_for_await.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "async_main_awaited_tuple_projected_task_tuple_for_await",
            source_relative: "ramdon_tests/async_program_surface_examples/130_async_main_awaited_tuple_projected_task_tuple_for_await.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "async_main_import_alias_awaited_aggregate_task_tuple_for_await",
            source_relative: "ramdon_tests/async_program_surface_examples/131_async_main_import_alias_awaited_aggregate_task_tuple_for_await.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "async_main_inline_awaited_task_tuple_for_await",
            source_relative: "ramdon_tests/async_program_surface_examples/132_async_main_inline_awaited_task_tuple_for_await.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "async_main_awaited_nested_aggregate_task_tuple_for_await",
            source_relative: "ramdon_tests/async_program_surface_examples/133_async_main_awaited_nested_aggregate_task_tuple_for_await.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "async_main_awaited_array_projected_task_tuple_for_await",
            source_relative: "ramdon_tests/async_program_surface_examples/134_async_main_awaited_array_projected_task_tuple_for_await.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "async_main_import_alias_task_tuple_for_await",
            source_relative: "ramdon_tests/async_program_surface_examples/135_async_main_import_alias_task_tuple_for_await.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "async_main_void_task_tuple_for_await",
            source_relative: "ramdon_tests/async_program_surface_examples/136_async_main_void_task_tuple_for_await.ql",
            expected_exit: 2,
        },
        ExecutableExampleCase {
            name: "async_main_const_tuple_for_await",
            source_relative: "ramdon_tests/async_program_surface_examples/137_async_main_const_tuple_for_await.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "async_main_static_array_for_await",
            source_relative: "ramdon_tests/async_program_surface_examples/138_async_main_static_array_for_await.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "async_main_import_alias_projected_const_tuple_for_await",
            source_relative: "ramdon_tests/async_program_surface_examples/139_async_main_import_alias_projected_const_tuple_for_await.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "async_main_import_alias_projected_static_array_for_await",
            source_relative: "ramdon_tests/async_program_surface_examples/140_async_main_import_alias_projected_static_array_for_await.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "async_main_import_alias_const_tuple_for_await",
            source_relative: "ramdon_tests/async_program_surface_examples/141_async_main_import_alias_const_tuple_for_await.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "async_main_import_alias_static_array_for_await",
            source_relative: "ramdon_tests/async_program_surface_examples/142_async_main_import_alias_static_array_for_await.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "async_main_call_root_projected_task_handle_consumes",
            source_relative: "ramdon_tests/async_program_surface_examples/143_async_main_call_root_projected_task_handle_consumes.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "async_main_call_root_fixed_shape_for_await",
            source_relative: "ramdon_tests/async_program_surface_examples/144_async_main_call_root_fixed_shape_for_await.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "async_main_import_alias_call_root_fixed_shape_for_await",
            source_relative: "ramdon_tests/async_program_surface_examples/145_async_main_import_alias_call_root_fixed_shape_for_await.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "async_main_nested_call_root_fixed_shape_for_await",
            source_relative: "ramdon_tests/async_program_surface_examples/146_async_main_nested_call_root_fixed_shape_for_await.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "async_main_import_alias_nested_call_root_fixed_shape_for_await",
            source_relative: "ramdon_tests/async_program_surface_examples/147_async_main_import_alias_nested_call_root_fixed_shape_for_await.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "async_main_import_alias_call_root_projected_task_handle_consumes",
            source_relative: "ramdon_tests/async_program_surface_examples/148_async_main_import_alias_call_root_projected_task_handle_consumes.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "async_main_import_alias_nested_call_root_projected_task_handle_consumes",
            source_relative: "ramdon_tests/async_program_surface_examples/149_async_main_import_alias_nested_call_root_projected_task_handle_consumes.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "async_main_import_alias_awaited_aggregate_projected_task_handle_consumes",
            source_relative: "ramdon_tests/async_program_surface_examples/150_async_main_import_alias_awaited_aggregate_projected_task_handle_consumes.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "async_main_awaited_aggregate_projected_task_handle_consumes",
            source_relative: "ramdon_tests/async_program_surface_examples/151_async_main_awaited_aggregate_projected_task_handle_consumes.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "async_main_inline_projected_fixed_shape_for_await",
            source_relative: "ramdon_tests/async_program_surface_examples/152_async_main_inline_projected_fixed_shape_for_await.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "async_main_inline_projected_task_handle_consumes",
            source_relative: "ramdon_tests/async_program_surface_examples/153_async_main_inline_projected_task_handle_consumes.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "async_main_import_alias_inline_projected_task_handle_consumes",
            source_relative: "ramdon_tests/async_program_surface_examples/154_async_main_import_alias_inline_projected_task_handle_consumes.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "async_main_import_alias_inline_projected_fixed_shape_for_await",
            source_relative: "ramdon_tests/async_program_surface_examples/155_async_main_import_alias_inline_projected_fixed_shape_for_await.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "async_main_awaited_aggregate_projected_fixed_shape_for_await",
            source_relative: "ramdon_tests/async_program_surface_examples/156_async_main_awaited_aggregate_projected_fixed_shape_for_await.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "async_main_import_alias_awaited_aggregate_projected_fixed_shape_for_await",
            source_relative: "ramdon_tests/async_program_surface_examples/157_async_main_import_alias_awaited_aggregate_projected_fixed_shape_for_await.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "async_main_nested_call_root_projected_task_handle_consumes",
            source_relative: "ramdon_tests/async_program_surface_examples/158_async_main_nested_call_root_projected_task_handle_consumes.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "async_main_inline_projected_fixed_shape_for_await_without_parens",
            source_relative: "ramdon_tests/async_program_surface_examples/159_async_main_inline_projected_fixed_shape_for_await_without_parens.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "async_main_awaited_match_guards",
            source_relative: "ramdon_tests/async_program_surface_examples/160_async_main_awaited_match_guards.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "async_main_awaited_match_aggregate_guard_calls",
            source_relative: "ramdon_tests/async_program_surface_examples/161_async_main_awaited_match_aggregate_guard_calls.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "async_main_import_alias_awaited_match_guards",
            source_relative: "ramdon_tests/async_program_surface_examples/162_async_main_import_alias_awaited_match_guards.ql",
            expected_exit: 62,
        },
        ExecutableExampleCase {
            name: "async_main_awaited_match_nested_call_root_guards",
            source_relative: "ramdon_tests/async_program_surface_examples/163_async_main_awaited_match_nested_call_root_guards.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "async_main_awaited_match_call_backed_nested_call_root_guards",
            source_relative: "ramdon_tests/async_program_surface_examples/164_async_main_awaited_match_call_backed_nested_call_root_guards.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "async_main_awaited_match_alias_backed_nested_call_root_guards",
            source_relative: "ramdon_tests/async_program_surface_examples/165_async_main_awaited_match_alias_backed_nested_call_root_guards.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "async_main_awaited_match_binding_backed_nested_call_root_guards",
            source_relative: "ramdon_tests/async_program_surface_examples/166_async_main_awaited_match_binding_backed_nested_call_root_guards.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "async_main_awaited_match_projection_backed_nested_call_root_guards",
            source_relative: "ramdon_tests/async_program_surface_examples/167_async_main_awaited_match_projection_backed_nested_call_root_guards.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "async_main_awaited_match_item_backed_nested_call_root_guards",
            source_relative: "ramdon_tests/async_program_surface_examples/168_async_main_awaited_match_item_backed_nested_call_root_guards.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "async_main_awaited_match_item_backed_inline_combos",
            source_relative: "ramdon_tests/async_program_surface_examples/169_async_main_awaited_match_item_backed_inline_combos.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "async_main_awaited_match_call_backed_combos",
            source_relative: "ramdon_tests/async_program_surface_examples/170_async_main_awaited_match_call_backed_combos.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "async_main_awaited_match_inline_aggregate_call_args",
            source_relative: "ramdon_tests/async_program_surface_examples/171_async_main_awaited_match_inline_aggregate_call_args.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "async_main_awaited_match_inline_projection_roots",
            source_relative: "ramdon_tests/async_program_surface_examples/172_async_main_awaited_match_inline_projection_roots.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "async_main_awaited_match_nested_call_root_inline_combos",
            source_relative: "ramdon_tests/async_program_surface_examples/173_async_main_awaited_match_nested_call_root_inline_combos.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "async_main_spawned_aggregate_results",
            source_relative: "ramdon_tests/async_program_surface_examples/174_async_main_spawned_aggregate_results.ql",
            expected_exit: 21,
        },
        ExecutableExampleCase {
            name: "async_main_zero_sized_and_recursive_aggregate_results",
            source_relative: "ramdon_tests/async_program_surface_examples/175_async_main_zero_sized_and_recursive_aggregate_results.ql",
            expected_exit: 22,
        },
        ExecutableExampleCase {
            name: "async_main_zero_sized_helper_task_handle_flows",
            source_relative: "ramdon_tests/async_program_surface_examples/176_async_main_zero_sized_helper_task_handle_flows.ql",
            expected_exit: 5,
        },
        ExecutableExampleCase {
            name: "async_main_zero_sized_projected_task_handle_consumes",
            source_relative: "ramdon_tests/async_program_surface_examples/177_async_main_zero_sized_projected_task_handle_consumes.ql",
            expected_exit: 6,
        },
        ExecutableExampleCase {
            name: "async_main_zero_sized_projected_task_handle_reinits",
            source_relative: "ramdon_tests/async_program_surface_examples/178_async_main_zero_sized_projected_task_handle_reinits.ql",
            expected_exit: 7,
        },
        ExecutableExampleCase {
            name: "async_main_zero_sized_conditional_task_handle_flows",
            source_relative: "ramdon_tests/async_program_surface_examples/179_async_main_zero_sized_conditional_task_handle_flows.ql",
            expected_exit: 6,
        },
        ExecutableExampleCase {
            name: "async_main_zero_sized_returned_task_handle_shapes",
            source_relative: "ramdon_tests/async_program_surface_examples/180_async_main_zero_sized_returned_task_handle_shapes.ql",
            expected_exit: 5,
        },
        ExecutableExampleCase {
            name: "async_main_zero_sized_aggregate_params",
            source_relative: "ramdon_tests/async_program_surface_examples/181_async_main_zero_sized_aggregate_params.ql",
            expected_exit: 14,
        },
        ExecutableExampleCase {
            name: "async_main_zero_sized_call_root_projected_task_handle_consumes",
            source_relative: "ramdon_tests/async_program_surface_examples/182_async_main_zero_sized_call_root_projected_task_handle_consumes.ql",
            expected_exit: 6,
        },
        ExecutableExampleCase {
            name: "async_main_import_alias_zero_sized_projected_task_handle_consumes",
            source_relative: "ramdon_tests/async_program_surface_examples/183_async_main_import_alias_zero_sized_projected_task_handle_consumes.ql",
            expected_exit: 6,
        },
        ExecutableExampleCase {
            name: "async_main_zero_sized_inline_projected_task_handle_consumes",
            source_relative: "ramdon_tests/async_program_surface_examples/184_async_main_zero_sized_inline_projected_task_handle_consumes.ql",
            expected_exit: 4,
        },
        ExecutableExampleCase {
            name: "async_main_zero_sized_nested_call_root_projected_task_handle_consumes",
            source_relative: "ramdon_tests/async_program_surface_examples/185_async_main_zero_sized_nested_call_root_projected_task_handle_consumes.ql",
            expected_exit: 4,
        },
        ExecutableExampleCase {
            name: "async_main_recursive_aggregate_params",
            source_relative: "ramdon_tests/async_program_surface_examples/186_async_main_recursive_aggregate_params.ql",
            expected_exit: 6,
        },
        ExecutableExampleCase {
            name: "async_main_spawned_recursive_aggregate_params",
            source_relative: "ramdon_tests/async_program_surface_examples/187_async_main_spawned_recursive_aggregate_params.ql",
            expected_exit: 6,
        },
        ExecutableExampleCase {
            name: "async_main_conditional_task_handle_flows",
            source_relative: "ramdon_tests/async_program_surface_examples/188_async_main_conditional_task_handle_flows.ql",
            expected_exit: 6,
        },
        ExecutableExampleCase {
            name: "async_main_spawn_bound_task_handles",
            source_relative: "ramdon_tests/async_program_surface_examples/189_async_main_spawn_bound_task_handles.ql",
            expected_exit: 3,
        },
        ExecutableExampleCase {
            name: "async_main_returned_task_handle_shapes",
            source_relative: "ramdon_tests/async_program_surface_examples/190_async_main_returned_task_handle_shapes.ql",
            expected_exit: 21,
        },
        ExecutableExampleCase {
            name: "async_main_projected_task_handle_reinit_families",
            source_relative: "ramdon_tests/async_program_surface_examples/191_async_main_projected_task_handle_reinit_families.ql",
            expected_exit: 50,
        },
        ExecutableExampleCase {
            name: "async_main_guard_refined_dynamic_path_families",
            source_relative: "ramdon_tests/async_program_surface_examples/192_async_main_guard_refined_dynamic_path_families.ql",
            expected_exit: 40,
        },
        ExecutableExampleCase {
            name: "async_main_static_alias_projected_root_dynamic_reinit_families",
            source_relative: "ramdon_tests/async_program_surface_examples/193_async_main_static_alias_projected_root_dynamic_reinit_families.ql",
            expected_exit: 42,
        },
        ExecutableExampleCase {
            name: "async_main_helper_task_handle_flows",
            source_relative: "ramdon_tests/async_program_surface_examples/194_async_main_helper_task_handle_flows.ql",
            expected_exit: 28,
        },
        ExecutableExampleCase {
            name: "async_main_task_handle_payload_families",
            source_relative: "ramdon_tests/async_program_surface_examples/195_async_main_task_handle_payload_families.ql",
            expected_exit: 61,
        },
        ExecutableExampleCase {
            name: "async_main_aggregate_param_families",
            source_relative: "ramdon_tests/async_program_surface_examples/196_async_main_aggregate_param_families.ql",
            expected_exit: 136,
        },
        ExecutableExampleCase {
            name: "async_main_aggregate_result_families",
            source_relative: "ramdon_tests/async_program_surface_examples/197_async_main_aggregate_result_families.ql",
            expected_exit: 62,
        },
        ExecutableExampleCase {
            name: "async_main_dynamic_task_handle_path_families",
            source_relative: "ramdon_tests/async_program_surface_examples/198_async_main_dynamic_task_handle_path_families.ql",
            expected_exit: 55,
        },
        ExecutableExampleCase {
            name: "async_main_aliased_projected_root_repackage_families",
            source_relative: "ramdon_tests/async_program_surface_examples/199_async_main_aliased_projected_root_repackage_families.ql",
            expected_exit: 96,
        },
        ExecutableExampleCase {
            name: "async_main_aliased_projected_root_spawn_families",
            source_relative: "ramdon_tests/async_program_surface_examples/200_async_main_aliased_projected_root_spawn_families.ql",
            expected_exit: 164,
        },
        ExecutableExampleCase {
            name: "async_unsafe_function_bodies",
            source_relative: "ramdon_tests/async_program_surface_examples/201_async_unsafe_function_bodies.ql",
            expected_exit: 7,
        },
        ExecutableExampleCase {
            name: "async_assignment_expressions",
            source_relative: "ramdon_tests/async_program_surface_examples/202_async_assignment_expressions.ql",
            expected_exit: 27,
        },
        ExecutableExampleCase {
            name: "async_dynamic_task_array_assignments",
            source_relative: "ramdon_tests/async_program_surface_examples/203_async_dynamic_task_array_assignments.ql",
            expected_exit: 11,
        },
        ExecutableExampleCase {
            name: "async_tuple_assignment_expressions",
            source_relative: "ramdon_tests/async_program_surface_examples/204_async_tuple_assignment_expressions.ql",
            expected_exit: 19,
        },
        ExecutableExampleCase {
            name: "async_projected_root_dynamic_task_array_assignments",
            source_relative: "ramdon_tests/async_program_surface_examples/205_async_projected_root_dynamic_task_array_assignments.ql",
            expected_exit: 12,
        },
        ExecutableExampleCase {
            name: "async_local_assignment_expressions",
            source_relative: "ramdon_tests/async_program_surface_examples/206_async_local_assignment_expressions.ql",
            expected_exit: 31,
        },
        ExecutableExampleCase {
            name: "async_projected_root_tuple_assignment_expressions",
            source_relative: "ramdon_tests/async_program_surface_examples/207_async_projected_root_tuple_assignment_expressions.ql",
            expected_exit: 19,
        },
        ExecutableExampleCase {
            name: "async_scalar_dynamic_array_assignments",
            source_relative: "ramdon_tests/async_program_surface_examples/208_async_scalar_dynamic_array_assignments.ql",
            expected_exit: 23,
        },
        ExecutableExampleCase {
            name: "async_projected_root_assignment_expressions",
            source_relative: "ramdon_tests/async_program_surface_examples/209_async_projected_root_assignment_expressions.ql",
            expected_exit: 13,
        },
        ExecutableExampleCase {
            name: "async_dynamic_assignment_expressions",
            source_relative: "ramdon_tests/async_program_surface_examples/210_async_dynamic_assignment_expressions.ql",
            expected_exit: 46,
        },
        ExecutableExampleCase {
            name: "async_nested_projected_dynamic_assignment_expressions",
            source_relative: "ramdon_tests/async_program_surface_examples/211_async_nested_projected_dynamic_assignment_expressions.ql",
            expected_exit: 20,
        },
        ExecutableExampleCase {
            name: "async_nested_projected_tuple_assignment_expressions",
            source_relative: "ramdon_tests/async_program_surface_examples/212_async_nested_projected_tuple_assignment_expressions.ql",
            expected_exit: 22,
        },
        ExecutableExampleCase {
            name: "async_nested_projected_assignment_expressions",
            source_relative: "ramdon_tests/async_program_surface_examples/213_async_nested_projected_assignment_expressions.ql",
            expected_exit: 16,
        },
        ExecutableExampleCase {
            name: "async_call_root_nested_projected_assignment_expressions",
            source_relative: "ramdon_tests/async_program_surface_examples/214_async_call_root_nested_projected_assignment_expressions.ql",
            expected_exit: 14,
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
