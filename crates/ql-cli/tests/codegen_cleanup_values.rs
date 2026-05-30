mod support;

use support::codegen::{PassCase, run_pass_case};
use support::workspace_root;

struct CleanupValueCase {
    name: &'static str,
    source_relative: &'static str,
    context: &'static str,
}

#[test]
fn cleanup_value_codegen_cases_match() {
    let workspace_root = workspace_root();
    let mut failures = Vec::new();

    for case in cleanup_value_cases() {
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
        "cleanup value codegen regressions:\n\n{}",
        failures.join("\n\n")
    );
}

fn cleanup_value_cases() -> &'static [CleanupValueCase] {
    &[
        CleanupValueCase {
            name: "direct_cleanup_build",
            source_relative: "fixtures/codegen/pass/cleanup_direct_call.ql",
            context: "direct cleanup build regression",
        },
        CleanupValueCase {
            name: "cleanup_block_assignment_expr_build",
            source_relative: "fixtures/codegen/pass/cleanup_block_assignment_expr.ql",
            context: "cleanup block assignment-expr build regression",
        },
        CleanupValueCase {
            name: "cleanup_value_assignment_expr_build",
            source_relative: "fixtures/codegen/pass/cleanup_value_assignment_expr.ql",
            context: "cleanup value assignment-expr build regression",
        },
        CleanupValueCase {
            name: "cleanup_if_value_build",
            source_relative: "fixtures/codegen/pass/cleanup_if_value.ql",
            context: "cleanup if-value build regression",
        },
        CleanupValueCase {
            name: "cleanup_match_value_build",
            source_relative: "fixtures/codegen/pass/cleanup_match_value.ql",
            context: "cleanup match-value build regression",
        },
        CleanupValueCase {
            name: "assignment_expr_value_build",
            source_relative: "fixtures/codegen/pass/assignment_expr_value.ql",
            context: "assignment expr value build regression",
        },
        CleanupValueCase {
            name: "guard_assignment_expr_build",
            source_relative: "fixtures/codegen/pass/guard_assignment_expr.ql",
            context: "guard assignment expr build regression",
        },
        CleanupValueCase {
            name: "guard_assignment_call_arg_build",
            source_relative: "fixtures/codegen/pass/guard_assignment_call_arg.ql",
            context: "guard assignment call-arg build regression",
        },
        CleanupValueCase {
            name: "guard_if_value_call_arg_build",
            source_relative: "fixtures/codegen/pass/guard_if_value_call_arg.ql",
            context: "guard if-value call-arg build regression",
        },
        CleanupValueCase {
            name: "guard_match_value_call_arg_build",
            source_relative: "fixtures/codegen/pass/guard_match_value_call_arg.ql",
            context: "guard match-value call-arg build regression",
        },
        CleanupValueCase {
            name: "guard_match_callable_callee_build",
            source_relative: "fixtures/codegen/pass/guard_match_callable_callee.ql",
            context: "guard match-callable callee build regression",
        },
        CleanupValueCase {
            name: "cleanup_await_value_build",
            source_relative: "fixtures/codegen/pass/cleanup_await_value.ql",
            context: "cleanup await-value build regression",
        },
        CleanupValueCase {
            name: "cleanup_spawn_value_build",
            source_relative: "fixtures/codegen/pass/cleanup_spawn_value.ql",
            context: "cleanup spawn-value build regression",
        },
        CleanupValueCase {
            name: "cleanup_await_guards_build",
            source_relative: "fixtures/codegen/pass/cleanup_await_guards.ql",
            context: "cleanup await-guard build regression",
        },
        CleanupValueCase {
            name: "cleanup_awaited_control_flow_roots_build",
            source_relative: "fixtures/codegen/pass/cleanup_awaited_control_flow_roots.ql",
            context: "cleanup awaited control-flow root build regression",
        },
    ]
}
