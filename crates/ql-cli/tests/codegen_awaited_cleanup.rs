mod support;

use support::codegen::{PassCase, run_pass_case};
use support::workspace_root;

struct AwaitedCleanupCase {
    name: &'static str,
    source_relative: &'static str,
    context: &'static str,
}

#[test]
fn awaited_cleanup_codegen_cases_match() {
    let workspace_root = workspace_root();
    let mut failures = Vec::new();

    for case in awaited_cleanup_cases() {
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
        "awaited cleanup codegen regressions:\n\n{}",
        failures.join("\n\n")
    );
}

fn awaited_cleanup_cases() -> &'static [AwaitedCleanupCase] {
    &[
        AwaitedCleanupCase {
            name: "cleanup_awaited_projection_async_callable_control_flow_scrutinees_build",
            source_relative: "fixtures/codegen/pass/cleanup_awaited_projection_async_callable_control_flow_scrutinees.ql",
            context: "cleanup awaited projection async callable control-flow scrutinee build regression",
        },
        AwaitedCleanupCase {
            name: "cleanup_awaited_helper_inline_guards_build",
            source_relative: "fixtures/codegen/pass/cleanup_awaited_helper_inline_guards.ql",
            context: "cleanup awaited helper/inline guard build regression",
        },
        AwaitedCleanupCase {
            name: "cleanup_awaited_nested_runtime_projection_guards_build",
            source_relative: "fixtures/codegen/pass/cleanup_awaited_nested_runtime_projection_guards.ql",
            context: "cleanup awaited nested runtime projection guard build regression",
        },
        AwaitedCleanupCase {
            name: "cleanup_awaited_helper_inline_scrutinees_build",
            source_relative: "fixtures/codegen/pass/cleanup_awaited_helper_inline_scrutinees.ql",
            context: "cleanup awaited helper/inline scrutinee build regression",
        },
        AwaitedCleanupCase {
            name: "cleanup_awaited_nested_runtime_projection_scrutinees_build",
            source_relative: "fixtures/codegen/pass/cleanup_awaited_nested_runtime_projection_scrutinees.ql",
            context: "cleanup awaited nested runtime projection scrutinee build regression",
        },
        AwaitedCleanupCase {
            name: "cleanup_awaited_aggregate_binding_scrutinees_build",
            source_relative: "fixtures/codegen/pass/cleanup_awaited_aggregate_binding_scrutinees.ql",
            context: "cleanup awaited aggregate binding scrutinee build regression",
        },
        AwaitedCleanupCase {
            name: "cleanup_awaited_aggregate_destructuring_scrutinees_build",
            source_relative: "fixtures/codegen/pass/cleanup_awaited_aggregate_destructuring_scrutinees.ql",
            context: "cleanup awaited aggregate destructuring scrutinee build regression",
        },
        AwaitedCleanupCase {
            name: "awaited_aggregate_destructuring_scrutinees_build",
            source_relative: "fixtures/codegen/pass/awaited_aggregate_destructuring_scrutinees.ql",
            context: "awaited aggregate destructuring scrutinee build regression",
        },
        AwaitedCleanupCase {
            name: "awaited_fixed_array_destructuring_scrutinees_build",
            source_relative: "fixtures/codegen/pass/awaited_fixed_array_destructuring_scrutinees.ql",
            context: "awaited fixed-array destructuring scrutinee build regression",
        },
        AwaitedCleanupCase {
            name: "fixed_array_bind_patterns_build",
            source_relative: "fixtures/codegen/pass/fixed_array_bind_patterns.ql",
            context: "fixed-array bind-pattern build regression",
        },
        AwaitedCleanupCase {
            name: "function_value_local_call_build",
            source_relative: "fixtures/codegen/pass/function_value_local_call.ql",
            context: "function value local call build regression",
        },
    ]
}
