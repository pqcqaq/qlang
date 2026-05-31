mod support;

use support::codegen::{PassCase, run_pass_case};
use support::workspace_root;

struct CleanupCallableMatchCase {
    name: &'static str,
    source_relative: &'static str,
    context: &'static str,
}

#[test]
fn cleanup_callable_match_codegen_cases_match() {
    let workspace_root = workspace_root();
    let mut failures = Vec::new();

    for case in cleanup_callable_match_cases() {
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
        "cleanup callable/match codegen regressions:\n\n{}",
        failures.join("\n\n")
    );
}

fn cleanup_callable_match_cases() -> &'static [CleanupCallableMatchCase] {
    &[
        CleanupCallableMatchCase {
            name: "callable_const_static_value_build",
            source_relative: "fixtures/codegen/pass/callable_const_static_value.ql",
            context: "callable const/static value build regression",
        },
        CleanupCallableMatchCase {
            name: "cleanup_callable_const_alias_build",
            source_relative: "fixtures/codegen/pass/cleanup_callable_const_alias.ql",
            context: "cleanup callable const alias build regression",
        },
        CleanupCallableMatchCase {
            name: "cleanup_foldable_function_item_calls_build",
            source_relative: "fixtures/codegen/pass/cleanup_foldable_function_item_calls.ql",
            context: "cleanup foldable function-item call build regression",
        },
        CleanupCallableMatchCase {
            name: "closure_backed_callable_cleanup_guard_build",
            source_relative: "fixtures/codegen/pass/closure_backed_callable_cleanup_guard_build.ql",
            context: "closure-backed callable cleanup/guard build regression",
        },
        CleanupCallableMatchCase {
            name: "local_closure_cleanup_guard_build",
            source_relative: "fixtures/codegen/pass/local_closure_cleanup_guard_build.ql",
            context: "local closure cleanup/guard build regression",
        },
        CleanupCallableMatchCase {
            name: "match_guard_callable_alias_build",
            source_relative: "fixtures/codegen/pass/match_guard_callable_alias.ql",
            context: "match guard callable alias build regression",
        },
        CleanupCallableMatchCase {
            name: "cleanup_match_build",
            source_relative: "fixtures/codegen/pass/cleanup_match_call.ql",
            context: "cleanup match build regression",
        },
        CleanupCallableMatchCase {
            name: "cleanup_match_callable_guard_alias_build",
            source_relative: "fixtures/codegen/pass/cleanup_match_callable_guard_alias.ql",
            context: "cleanup match callable guard alias build regression",
        },
        CleanupCallableMatchCase {
            name: "cleanup_string_match_build",
            source_relative: "fixtures/codegen/pass/cleanup_string_match_build.ql",
            context: "cleanup string match build regression",
        },
        CleanupCallableMatchCase {
            name: "cleanup_match_binding_arm_build",
            source_relative: "fixtures/codegen/pass/cleanup_match_binding_arm.ql",
            context: "cleanup match binding arm build regression",
        },
        CleanupCallableMatchCase {
            name: "cleanup_branch_async_blocks_build",
            source_relative: "fixtures/codegen/pass/cleanup_branch_async_blocks.ql",
            context: "cleanup async branch-block build regression",
        },
    ]
}
