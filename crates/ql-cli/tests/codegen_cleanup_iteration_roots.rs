mod support;

use support::codegen::{PassCase, run_pass_case};
use support::workspace_root;

struct CleanupIterationCase {
    name: &'static str,
    source_relative: &'static str,
    context: &'static str,
}

#[test]
fn cleanup_iteration_root_codegen_cases_match() {
    let workspace_root = workspace_root();
    let mut failures = Vec::new();

    for case in cleanup_iteration_cases() {
        let pass_case = PassCase {
            name: case.name,
            source_relative: case.source_relative,
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        };

        if let Err(message) = run_pass_case(&workspace_root, &pass_case) {
            failures.push(format!("{}:\n\n{}", case.context, message));
        }
    }

    assert!(
        failures.is_empty(),
        "cleanup iteration root codegen regressions:\n\n{}",
        failures.join("\n\n")
    );
}

fn cleanup_iteration_cases() -> &'static [CleanupIterationCase] {
    &[
        CleanupIterationCase {
            name: "cleanup_block_for_fixed_shapes_build",
            source_relative: "fixtures/codegen/pass/cleanup_block_for_fixed_shapes.ql",
            context: "cleanup block for fixed-shape build regression",
        },
        CleanupIterationCase {
            name: "cleanup_block_for_await_fixed_shapes_build",
            source_relative: "fixtures/codegen/pass/cleanup_block_for_await_fixed_shapes.ql",
            context: "cleanup block for-await fixed-shape build regression",
        },
        CleanupIterationCase {
            name: "cleanup_block_for_await_call_roots_build",
            source_relative: "fixtures/codegen/pass/cleanup_block_for_await_call_roots.ql",
            context: "cleanup block for-await call-root build regression",
        },
        CleanupIterationCase {
            name: "cleanup_block_for_await_direct_control_flow_roots_build",
            source_relative: "fixtures/codegen/pass/cleanup_block_for_await_direct_control_flow_roots.ql",
            context: "cleanup block for-await direct control-flow roots build regression",
        },
        CleanupIterationCase {
            name: "cleanup_block_for_await_direct_question_roots_build",
            source_relative: "fixtures/codegen/pass/cleanup_block_for_await_direct_question_roots.ql",
            context: "cleanup block for-await direct question-root build regression",
        },
        CleanupIterationCase {
            name: "cleanup_block_for_await_scalar_item_roots_build",
            source_relative: "fixtures/codegen/pass/cleanup_block_for_await_scalar_item_roots.ql",
            context: "cleanup block for-await scalar item-root build regression",
        },
        CleanupIterationCase {
            name: "task_item_roots_for_await_and_cleanup_build",
            source_relative: "fixtures/codegen/pass/task_item_roots_for_await_and_cleanup.ql",
            context: "task item-root for-await build regression",
        },
        CleanupIterationCase {
            name: "projected_task_item_roots_for_await_and_cleanup_build",
            source_relative: "fixtures/codegen/pass/projected_task_item_roots_for_await_and_cleanup.ql",
            context: "projected task item-root for-await build regression",
        },
        CleanupIterationCase {
            name: "task_item_value_flow_in_async_builds_build",
            source_relative: "fixtures/codegen/pass/task_item_value_flow_in_async_builds.ql",
            context: "task item-value async flow build regression",
        },
        CleanupIterationCase {
            name: "callable_value_control_flow_in_async_and_cleanup_build",
            source_relative: "fixtures/codegen/pass/callable_value_control_flow_in_async_and_cleanup.ql",
            context: "callable value control-flow async/cleanup build regression",
        },
        CleanupIterationCase {
            name: "cleanup_block_for_await_inline_task_roots_build",
            source_relative: "fixtures/codegen/pass/cleanup_block_for_await_inline_task_roots.ql",
            context: "cleanup block for-await inline-task roots build regression",
        },
        CleanupIterationCase {
            name: "cleanup_block_for_await_awaited_projected_root_build",
            source_relative: "fixtures/codegen/pass/cleanup_block_for_await_awaited_projected_root.ql",
            context: "cleanup block for-await awaited-projected root build regression",
        },
        CleanupIterationCase {
            name: "cleanup_block_let_struct_literal_with_awaited_projected_field_build",
            source_relative: "fixtures/codegen/pass/cleanup_block_let_struct_literal_with_awaited_projected_field.ql",
            context: "cleanup block let-struct awaited-projected field build regression",
        },
        CleanupIterationCase {
            name: "cleanup_block_for_await_projected_if_match_roots_build",
            source_relative: "fixtures/codegen/pass/cleanup_block_for_await_projected_if_match_roots.ql",
            context: "cleanup block for-await projected if/match roots build regression",
        },
        CleanupIterationCase {
            name: "cleanup_block_for_await_projected_block_root_build",
            source_relative: "fixtures/codegen/pass/cleanup_block_for_await_projected_block_root.ql",
            context: "cleanup block for-await projected block-root build regression",
        },
        CleanupIterationCase {
            name: "cleanup_block_for_await_projected_assignment_root_build",
            source_relative: "fixtures/codegen/pass/cleanup_block_for_await_projected_assignment_root.ql",
            context: "cleanup block for-await projected assignment-root build regression",
        },
        CleanupIterationCase {
            name: "cleanup_block_for_await_projected_question_root_build",
            source_relative: "fixtures/codegen/pass/cleanup_block_for_await_projected_question_root.ql",
            context: "cleanup block for-await projected question-root build regression",
        },
    ]
}
