mod support;

use support::codegen::{PassCase, run_pass_case};
use support::workspace_root;

struct CapturingClosureTaskCase {
    name: &'static str,
    source_relative: &'static str,
    context: &'static str,
}

#[test]
fn capturing_closure_task_codegen_cases_match() {
    let workspace_root = workspace_root();
    let mut failures = Vec::new();

    for case in capturing_closure_task_cases() {
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
        "capturing closure task codegen regressions:\n\n{}",
        failures.join("\n\n")
    );
}

fn capturing_closure_task_cases() -> &'static [CapturingClosureTaskCase] {
    &[
        CapturingClosureTaskCase {
            name: "capturing_closure_direct_local_call_build",
            source_relative: "fixtures/codegen/pass/capturing_closure_direct_local_call.ql",
            context: "capturing closure direct local call build regression",
        },
        CapturingClosureTaskCase {
            name: "capturing_closure_direct_local_string_call_build",
            source_relative: "fixtures/codegen/pass/capturing_closure_direct_local_string_call.ql",
            context: "capturing closure direct local string call build regression",
        },
        CapturingClosureTaskCase {
            name: "capturing_closure_task_handle_await_call_build",
            source_relative: "fixtures/codegen/pass/capturing_closure_task_handle_await_call.ql",
            context: "capturing closure task-handle await call build regression",
        },
        CapturingClosureTaskCase {
            name: "capturing_closure_task_handle_control_flow_await_call_build",
            source_relative: "fixtures/codegen/pass/capturing_closure_task_handle_control_flow_await_call.ql",
            context: "capturing closure task-handle control-flow await call build regression",
        },
        CapturingClosureTaskCase {
            name: "capturing_closure_task_handle_cleanup_await_build",
            source_relative: "fixtures/codegen/pass/capturing_closure_task_handle_cleanup_await.ql",
            context: "capturing closure task-handle cleanup await build regression",
        },
        CapturingClosureTaskCase {
            name: "capturing_closure_task_handle_cleanup_await_root_matrix_build",
            source_relative: "fixtures/codegen/pass/capturing_closure_task_handle_cleanup_await_root_matrix.ql",
            context: "capturing closure task-handle cleanup await root-matrix build regression",
        },
        CapturingClosureTaskCase {
            name: "capturing_closure_task_handle_cleanup_await_alias_roots_build",
            source_relative: "fixtures/codegen/pass/capturing_closure_task_handle_cleanup_await_alias_roots.ql",
            context: "capturing closure task-handle cleanup await alias-root build regression",
        },
        CapturingClosureTaskCase {
            name: "capturing_closure_task_handle_cleanup_await_helper_inline_values_build",
            source_relative: "fixtures/codegen/pass/capturing_closure_task_handle_cleanup_await_helper_inline_values.ql",
            context: "capturing closure task-handle cleanup await helper/inline values build regression",
        },
        CapturingClosureTaskCase {
            name: "capturing_closure_task_handle_cleanup_await_nested_runtime_projection_values_build",
            source_relative: "fixtures/codegen/pass/capturing_closure_task_handle_cleanup_await_nested_runtime_projection_values.ql",
            context: "capturing closure task-handle cleanup await nested runtime projection values build regression",
        },
        CapturingClosureTaskCase {
            name: "capturing_closure_task_handle_cleanup_await_aggregate_binding_scrutinees_build",
            source_relative: "fixtures/codegen/pass/capturing_closure_task_handle_cleanup_await_aggregate_binding_scrutinees.ql",
            context: "capturing closure task-handle cleanup await aggregate binding scrutinees build regression",
        },
        CapturingClosureTaskCase {
            name: "capturing_closure_task_handle_cleanup_await_aggregate_destructuring_scrutinees_build",
            source_relative: "fixtures/codegen/pass/capturing_closure_task_handle_cleanup_await_aggregate_destructuring_scrutinees.ql",
            context: "capturing closure task-handle cleanup await aggregate destructuring scrutinees build regression",
        },
        CapturingClosureTaskCase {
            name: "capturing_closure_task_handle_cleanup_await_fixed_array_destructuring_scrutinees_build",
            source_relative: "fixtures/codegen/pass/capturing_closure_task_handle_cleanup_await_fixed_array_destructuring_scrutinees.ql",
            context: "capturing closure task-handle cleanup await fixed-array destructuring scrutinees build regression",
        },
        CapturingClosureTaskCase {
            name: "capturing_closure_task_handle_cleanup_await_different_closure_roots_build",
            source_relative: "fixtures/codegen/pass/capturing_closure_task_handle_cleanup_await_different_closure_roots.ql",
            context: "capturing closure task-handle cleanup await different-closure roots build regression",
        },
        CapturingClosureTaskCase {
            name: "capturing_closure_task_handle_cleanup_await_different_closure_alias_roots_build",
            source_relative: "fixtures/codegen/pass/capturing_closure_task_handle_cleanup_await_different_closure_alias_roots.ql",
            context: "capturing closure task-handle cleanup await different-closure alias roots build regression",
        },
        CapturingClosureTaskCase {
            name: "capturing_closure_task_handle_cleanup_await_shared_local_alias_chains_build",
            source_relative: "fixtures/codegen/pass/capturing_closure_task_handle_cleanup_await_shared_local_alias_chains.ql",
            context: "capturing closure task-handle cleanup await shared-local alias chains build regression",
        },
        CapturingClosureTaskCase {
            name: "capturing_closure_task_handle_cleanup_await_guarded_match_shared_local_alias_chains_build",
            source_relative: "fixtures/codegen/pass/capturing_closure_task_handle_cleanup_await_guarded_match_shared_local_alias_chains.ql",
            context: "capturing closure task-handle cleanup await guarded-match shared-local alias chains build regression",
        },
        CapturingClosureTaskCase {
            name: "capturing_closure_task_handle_cleanup_await_tagged_guarded_match_shared_local_alias_chains_build",
            source_relative: "fixtures/codegen/pass/capturing_closure_task_handle_cleanup_await_tagged_guarded_match_shared_local_alias_chains.ql",
            context: "capturing closure task-handle cleanup await tagged guarded-match shared-local alias chains build regression",
        },
        CapturingClosureTaskCase {
            name: "capturing_closure_task_handle_cleanup_await_tagged_guarded_match_different_closure_roots_build",
            source_relative: "fixtures/codegen/pass/capturing_closure_task_handle_cleanup_await_tagged_guarded_match_different_closure_roots.ql",
            context: "capturing closure task-handle cleanup await tagged guarded-match different-closure roots build regression",
        },
        CapturingClosureTaskCase {
            name: "capturing_closure_task_handle_cleanup_await_tagged_guarded_match_different_closure_alias_roots_build",
            source_relative: "fixtures/codegen/pass/capturing_closure_task_handle_cleanup_await_tagged_guarded_match_different_closure_alias_roots.ql",
            context: "capturing closure task-handle cleanup await tagged guarded-match different-closure alias roots build regression",
        },
        CapturingClosureTaskCase {
            name: "capturing_closure_task_handle_cleanup_await_guarded_match_different_closure_roots_build",
            source_relative: "fixtures/codegen/pass/capturing_closure_task_handle_cleanup_await_guarded_match_different_closure_roots.ql",
            context: "capturing closure task-handle cleanup await guarded-match different-closure roots build regression",
        },
        CapturingClosureTaskCase {
            name: "capturing_closure_task_handle_cleanup_await_guarded_match_different_closure_alias_roots_build",
            source_relative: "fixtures/codegen/pass/capturing_closure_task_handle_cleanup_await_guarded_match_different_closure_alias_roots.ql",
            context: "capturing closure task-handle cleanup await guarded-match different-closure alias roots build regression",
        },
    ]
}
