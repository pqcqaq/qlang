mod support;

use support::codegen::{PassCase, run_pass_case};
use support::workspace_root;

struct AwaitedControlFlowCase {
    name: &'static str,
    source_relative: &'static str,
    context: &'static str,
}

#[test]
fn awaited_control_flow_codegen_cases_match() {
    let workspace_root = workspace_root();
    let mut failures = Vec::new();

    for case in awaited_control_flow_cases() {
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
        "awaited control-flow codegen regressions:\n\n{}",
        failures.join("\n\n")
    );
}

fn awaited_control_flow_cases() -> &'static [AwaitedControlFlowCase] {
    &[
        AwaitedControlFlowCase {
            name: "callable_callee_control_flow_roots_build",
            source_relative: "fixtures/codegen/pass/callable_callee_control_flow_roots.ql",
            context: "callable callee control-flow root build regression",
        },
        AwaitedControlFlowCase {
            name: "awaited_guard_async_callable_control_flow_roots_build",
            source_relative: "fixtures/codegen/pass/awaited_guard_async_callable_control_flow_roots.ql",
            context: "awaited guard async callable control-flow root build regression",
        },
        AwaitedControlFlowCase {
            name: "awaited_scrutinee_async_callable_control_flow_roots_build",
            source_relative: "fixtures/codegen/pass/awaited_scrutinee_async_callable_control_flow_roots.ql",
            context: "awaited scrutinee async callable control-flow root build regression",
        },
        AwaitedControlFlowCase {
            name: "awaited_projection_async_callable_control_flow_scrutinees_build",
            source_relative: "fixtures/codegen/pass/awaited_projection_async_callable_control_flow_scrutinees.ql",
            context: "awaited projection async callable control-flow scrutinee build regression",
        },
        AwaitedControlFlowCase {
            name: "awaited_aggregate_binding_scrutinees_build",
            source_relative: "fixtures/codegen/pass/awaited_aggregate_binding_scrutinees.ql",
            context: "awaited aggregate binding scrutinee build regression",
        },
        AwaitedControlFlowCase {
            name: "awaited_projection_async_callable_control_flow_guard_build",
            source_relative: "fixtures/codegen/pass/awaited_projection_async_callable_control_flow_guard.ql",
            context: "awaited projection async callable control-flow guard build regression",
        },
        AwaitedControlFlowCase {
            name: "awaited_aggregate_guard_async_callable_control_flow_roots_build",
            source_relative: "fixtures/codegen/pass/awaited_aggregate_guard_async_callable_control_flow_roots.ql",
            context: "awaited aggregate guard async callable control-flow root build regression",
        },
        AwaitedControlFlowCase {
            name: "awaited_call_backed_aggregate_guard_async_callable_control_flow_roots_build",
            source_relative: "fixtures/codegen/pass/awaited_call_backed_aggregate_guard_async_callable_control_flow_roots.ql",
            context: "awaited call-backed aggregate guard async callable control-flow root build regression",
        },
        AwaitedControlFlowCase {
            name: "awaited_guard_import_alias_helpers_build",
            source_relative: "fixtures/codegen/pass/awaited_guard_import_alias_helpers.ql",
            context: "awaited guard import-alias helper build regression",
        },
        AwaitedControlFlowCase {
            name: "awaited_nested_call_root_runtime_projection_guards_build",
            source_relative: "fixtures/codegen/pass/awaited_nested_call_root_runtime_projection_guards.ql",
            context: "awaited nested call-root runtime projection guard build regression",
        },
        AwaitedControlFlowCase {
            name: "awaited_inline_guard_families_build",
            source_relative: "fixtures/codegen/pass/awaited_inline_guard_families.ql",
            context: "awaited inline guard family build regression",
        },
        AwaitedControlFlowCase {
            name: "awaited_scrutinee_families_build",
            source_relative: "fixtures/codegen/pass/awaited_scrutinee_families.ql",
            context: "awaited scrutinee family build regression",
        },
    ]
}
