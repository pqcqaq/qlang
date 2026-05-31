mod support;

use support::codegen::{PassCase, current_archiver_style, run_pass_case};
use support::workspace_root;

struct CleanupProjectedRootCase {
    name: &'static str,
    source_relative: &'static str,
    emit: &'static str,
    expected_relative: &'static str,
    mock_archiver: bool,
    uses_current_archiver_style: bool,
    context: &'static str,
}

#[test]
fn cleanup_projected_root_codegen_cases_match() {
    let workspace_root = workspace_root();
    let mut failures = Vec::new();

    for case in cleanup_projected_root_cases() {
        let pass_case = PassCase {
            name: case.name,
            source_relative: case.source_relative,
            emit: case.emit,
            expected_relative: case.expected_relative,
            mock_compiler: true,
            mock_archiver: case.mock_archiver,
            archiver_style: case
                .uses_current_archiver_style
                .then(current_archiver_style),
            header_surface: None,
            expected_header_relative: None,
        };

        if let Err(message) = run_pass_case(&workspace_root, &pass_case) {
            failures.push(format!("{}:\n\n{}", case.context, message));
        }
    }

    assert!(
        failures.is_empty(),
        "cleanup projected root codegen regressions:\n\n{}",
        failures.join("\n\n")
    );
}

fn cleanup_projected_root_cases() -> &'static [CleanupProjectedRootCase] {
    &[
        CleanupProjectedRootCase {
            name: "fixed_shape_for_projected_control_flow_roots_build",
            source_relative: "fixtures/codegen/pass/fixed_shape_for_projected_control_flow_roots.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_archiver: false,
            uses_current_archiver_style: false,
            context: "fixed-shape for projected control-flow roots build regression",
        },
        CleanupProjectedRootCase {
            name: "fixed_shape_for_await_projected_control_flow_roots_build",
            source_relative: "fixtures/codegen/pass/fixed_shape_for_await_projected_control_flow_roots.ql",
            emit: "staticlib",
            expected_relative: "tests/codegen/pass/minimal_library.staticlib.txt",
            mock_archiver: true,
            uses_current_archiver_style: true,
            context: "fixed-shape for-await projected control-flow roots build regression",
        },
        CleanupProjectedRootCase {
            name: "cleanup_block_for_projected_question_root_build",
            source_relative: "fixtures/codegen/pass/cleanup_block_for_projected_question_root.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_archiver: false,
            uses_current_archiver_style: false,
            context: "cleanup block for projected question-root build regression",
        },
        CleanupProjectedRootCase {
            name: "cleanup_block_for_destructuring_build",
            source_relative: "fixtures/codegen/pass/cleanup_block_for_destructuring.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_archiver: false,
            uses_current_archiver_style: false,
            context: "cleanup block destructuring for build regression",
        },
        CleanupProjectedRootCase {
            name: "cleanup_block_for_projected_call_roots_build",
            source_relative: "fixtures/codegen/pass/cleanup_block_for_projected_call_roots.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_archiver: false,
            uses_current_archiver_style: false,
            context: "cleanup block projected/call-root for build regression",
        },
        CleanupProjectedRootCase {
            name: "cleanup_block_for_alias_nested_call_roots_build",
            source_relative: "fixtures/codegen/pass/cleanup_block_for_alias_nested_call_roots.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_archiver: false,
            uses_current_archiver_style: false,
            context: "cleanup block alias/nested-call-root for build regression",
        },
        CleanupProjectedRootCase {
            name: "cleanup_block_for_const_static_roots_build",
            source_relative: "fixtures/codegen/pass/cleanup_block_for_const_static_roots.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_archiver: false,
            uses_current_archiver_style: false,
            context: "cleanup block const/static-root for build regression",
        },
        CleanupProjectedRootCase {
            name: "bind_pattern_destructuring_build",
            source_relative: "fixtures/codegen/pass/bind_pattern_destructuring.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_archiver: false,
            uses_current_archiver_style: false,
            context: "bind-pattern destructuring build regression",
        },
        CleanupProjectedRootCase {
            name: "cleanup_block_guard_scrutinee_value_build",
            source_relative: "fixtures/codegen/pass/cleanup_block_guard_scrutinee_value.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_archiver: false,
            uses_current_archiver_style: false,
            context: "cleanup block guard/scrutinee/value build regression",
        },
        CleanupProjectedRootCase {
            name: "cleanup_foldable_control_flow_values_build",
            source_relative: "fixtures/codegen/pass/cleanup_foldable_control_flow_values.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_archiver: false,
            uses_current_archiver_style: false,
            context: "cleanup foldable control-flow values build regression",
        },
    ]
}
