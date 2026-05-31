mod support;

use support::codegen::{PassCase, run_pass_case};
use support::workspace_root;

#[test]
fn async_ir_codegen_cases_match() {
    let workspace_root = workspace_root();
    let mut failures = Vec::new();

    for stem in async_ir_stems() {
        let source_relative = format!("fixtures/codegen/pass/{stem}.ql");
        let expected_relative = format!("tests/codegen/pass/{stem}.ll");
        let case_name = format!("{stem}_llvm_ir");
        let pass_case = PassCase {
            name: &case_name,
            source_relative: &source_relative,
            emit: "llvm-ir",
            expected_relative: &expected_relative,
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        };

        if let Err(message) = run_pass_case(&workspace_root, &pass_case) {
            failures.push(format!("{} LLVM IR regression:\n\n{}", stem, message));
        }
    }

    assert!(
        failures.is_empty(),
        "async LLVM IR codegen regressions:\n\n{}",
        failures.join("\n\n")
    );
}

fn async_ir_stems() -> &'static [&'static str] {
    &[
        "async_program_main",
        "async_program_main_spawn_bound_task_handle",
        "async_program_main_for_await_array",
        "async_program_main_guard_refined_dynamic_task_handle_reinit",
        "async_program_main_guard_refined_projected_dynamic_task_handle_reinit",
        "async_program_main_projected_root_dynamic_task_handle_reinit",
        "async_program_main_projected_root_const_backed_dynamic_task_handle_reinit",
        "async_program_main_aliased_projected_root_dynamic_task_handle_reinit",
        "async_program_main_aliased_projected_root_const_backed_dynamic_task_handle_reinit",
        "async_program_main_aliased_guard_refined_projected_root_dynamic_task_handle_reinit",
        "async_program_main_aliased_guard_refined_const_backed_projected_root_dynamic_task_handle_reinit",
        "async_program_main_aliased_guard_refined_static_alias_backed_projected_root_dynamic_task_handle_reinit",
        "async_program_main_aliased_projected_root_task_handle_tuple_repackage_reinit",
        "async_program_main_aliased_projected_root_task_handle_struct_repackage_reinit",
        "async_program_main_aliased_projected_root_task_handle_nested_repackage_reinit",
        "async_program_main_aliased_projected_root_task_handle_nested_repackage_spawn",
        "async_program_main_aliased_projected_root_task_handle_array_repackage_spawn",
        "async_program_main_aliased_projected_root_task_handle_nested_array_repackage_spawn",
        "async_program_main_aliased_guard_refined_const_backed_projected_root_task_handle_nested_array_repackage_spawn",
        "async_program_main_aliased_projected_root_task_handle_forwarded_nested_array_repackage_spawn",
        "async_program_main_aliased_guard_refined_const_backed_projected_root_task_handle_forwarded_nested_array_repackage_spawn",
        "async_program_main_composed_dynamic_task_handle_nested_array_repackage_spawn",
        "async_program_main_alias_sourced_composed_dynamic_task_handle_nested_array_repackage_spawn",
        "async_program_main_guarded_alias_sourced_composed_dynamic_task_handle_nested_array_repackage_spawn",
        "async_program_main_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn_with_tail_field",
        "async_program_main_guarded_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn_with_tail_field",
        "async_program_main_const_backed_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn",
        "async_program_main_guarded_const_backed_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn",
        "async_program_main_guarded_const_backed_double_root_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn",
        "async_program_main_guarded_const_backed_double_root_double_source_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn",
        "async_program_main_guarded_const_backed_double_root_double_source_row_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn",
        "async_program_main_guarded_const_backed_double_root_double_source_row_slot_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn",
        "async_program_main_guarded_const_backed_triple_root_double_source_row_slot_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn",
        "async_program_main_guarded_const_backed_triple_root_triple_source_row_slot_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn",
        "async_program_main_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn",
        "async_program_main_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_task_handle_forwarded_alias_nested_array_repackage_spawn",
        "async_program_main_guarded_const_backed_triple_root_triple_source_tail_queued_spawn",
        "async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_root_spawn",
        "async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_root_alias_spawn",
        "async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_root_chain_spawn",
        "async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_local_alias_spawn",
        "async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_local_chain_spawn",
        "async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_local_forward_spawn",
        "async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_local_inline_forward_spawn",
        "async_program_main_guarded_const_backed_triple_root_triple_source_tail_bundle_inline_forward_spawn",
        "async_program_main_guarded_const_backed_triple_root_triple_source_tail_bundle_slot_inline_forward_spawn",
        "async_program_main_guarded_const_backed_triple_root_triple_source_tail_direct_inline_forward_spawn",
        "async_program_main_guarded_const_backed_triple_root_triple_source_tail_direct_inline_forward_await",
        "async_program_main_guarded_const_backed_triple_root_triple_source_tail_bundle_slot_inline_forward_await",
        "async_program_main_guarded_const_backed_triple_root_triple_source_tail_bundle_inline_forward_await",
        "async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_local_inline_forward_await",
        "async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_local_forward_await",
        "async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_root_inline_forward_await",
        "async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_root_forward_await",
        "async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_root_alias_forward_await",
        "async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_root_chain_forward_await",
        "async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_root_alias_inline_forward_await",
        "async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_root_chain_inline_forward_await",
        "async_program_main_guarded_const_backed_triple_root_triple_source_tail_bundle_forward_await",
        "async_program_main_guarded_const_backed_triple_root_triple_source_tail_bundle_alias_forward_await",
        "async_program_main_guarded_const_backed_triple_root_triple_source_tail_bundle_chain_forward_await",
        "async_program_main_guarded_const_backed_triple_root_triple_source_tail_bundle_alias_inline_forward_await",
        "async_program_main_guarded_const_backed_triple_root_triple_source_tail_bundle_chain_inline_forward_await",
        "async_program_main_guarded_const_backed_triple_root_triple_source_tail_bundle_forward_spawn",
        "async_program_main_guarded_const_backed_triple_root_triple_source_tail_bundle_alias_forward_spawn",
        "async_program_main_guarded_const_backed_triple_root_triple_source_tail_bundle_chain_forward_spawn",
        "async_program_main_aliased_guard_refined_const_backed_projected_root_task_handle_nested_repackage_reinit",
        "async_program_main_aliased_guard_refined_const_backed_projected_root_task_handle_nested_repackage_spawn",
        "async_program_main_aliased_guard_refined_const_backed_projected_root_task_handle_array_repackage_spawn",
        "async_program_main_composed_dynamic_task_handle_reinit",
        "async_program_main_alias_sourced_composed_dynamic_task_handle_reinit",
    ]
}
