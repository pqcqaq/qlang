mod support;

use support::codegen::{PassCase, run_pass_case};
use support::workspace_root;

struct CapturingClosureRootCase {
    name: &'static str,
    source_relative: &'static str,
    context: &'static str,
}

#[test]
fn capturing_closure_root_codegen_cases_match() {
    let workspace_root = workspace_root();
    let mut failures = Vec::new();

    for case in capturing_closure_root_cases() {
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
        "capturing closure root codegen regressions:\n\n{}",
        failures.join("\n\n")
    );
}

fn capturing_closure_root_cases() -> &'static [CapturingClosureRootCase] {
    &[
        CapturingClosureRootCase {
            name: "capturing_closure_immutable_alias_call_build",
            source_relative: "fixtures/codegen/pass/capturing_closure_immutable_alias_call.ql",
            context: "capturing closure immutable alias call build regression",
        },
        CapturingClosureRootCase {
            name: "capturing_closure_mutable_alias_reassign_call_build",
            source_relative: "fixtures/codegen/pass/capturing_closure_mutable_alias_reassign_call_build.ql",
            context: "capturing closure mutable alias reassign build regression",
        },
        CapturingClosureRootCase {
            name: "capturing_closure_mutable_alias_cleanup_guard_build",
            source_relative: "fixtures/codegen/pass/capturing_closure_mutable_alias_cleanup_guard_build.ql",
            context: "capturing closure mutable alias cleanup/guard build regression",
        },
        CapturingClosureRootCase {
            name: "cleanup_block_different_target_mutable_capturing_closure_reassign_build",
            source_relative: "fixtures/codegen/pass/cleanup_block_different_target_mutable_capturing_closure_reassign_build.ql",
            context: "different-target mutable capturing-closure reassign build regression",
        },
        CapturingClosureRootCase {
            name: "capturing_closure_same_target_control_flow_build",
            source_relative: "fixtures/codegen/pass/capturing_closure_same_target_control_flow_build.ql",
            context: "capturing closure same-target control-flow build regression",
        },
        CapturingClosureRootCase {
            name: "capturing_closure_ordinary_extended_call_roots_build",
            source_relative: "fixtures/codegen/pass/capturing_closure_ordinary_extended_call_roots_build.ql",
            context: "capturing closure ordinary extended call-root build regression",
        },
        CapturingClosureRootCase {
            name: "capturing_closure_cleanup_guard_build",
            source_relative: "fixtures/codegen/pass/capturing_closure_cleanup_guard_build.ql",
            context: "capturing closure cleanup/guard build regression",
        },
        CapturingClosureRootCase {
            name: "capturing_closure_cleanup_if_match_guard_build",
            source_relative: "fixtures/codegen/pass/capturing_closure_cleanup_if_match_guard_build.ql",
            context: "capturing closure cleanup if/match guard build regression",
        },
        CapturingClosureRootCase {
            name: "capturing_closure_cleanup_different_closure_call_roots_build",
            source_relative: "fixtures/codegen/pass/capturing_closure_cleanup_different_closure_call_roots_build.ql",
            context: "capturing closure cleanup different-closure call-root build regression",
        },
        CapturingClosureRootCase {
            name: "capturing_closure_ordinary_different_closure_call_roots_build",
            source_relative: "fixtures/codegen/pass/capturing_closure_ordinary_different_closure_call_roots_build.ql",
            context: "capturing closure ordinary different-closure call-root build regression",
        },
        CapturingClosureRootCase {
            name: "capturing_closure_ordinary_different_closure_binding_roots_build",
            source_relative: "fixtures/codegen/pass/capturing_closure_ordinary_different_closure_binding_roots_build.ql",
            context: "capturing closure ordinary different-closure binding-root build regression",
        },
        CapturingClosureRootCase {
            name: "capturing_closure_ordinary_string_match_call_roots_build",
            source_relative: "fixtures/codegen/pass/capturing_closure_ordinary_string_match_call_roots_build.ql",
            context: "capturing closure ordinary string-match call-root build regression",
        },
        CapturingClosureRootCase {
            name: "capturing_closure_ordinary_guarded_string_match_call_roots_build",
            source_relative: "fixtures/codegen/pass/capturing_closure_ordinary_guarded_string_match_call_roots_build.ql",
            context: "capturing closure ordinary guarded string-match call-root build regression",
        },
        CapturingClosureRootCase {
            name: "capturing_closure_match_guard_control_flow_local_alias_build",
            source_relative: "fixtures/codegen/pass/capturing_closure_match_guard_control_flow_local_alias_build.ql",
            context: "capturing closure match-guard control-flow local-alias build regression",
        },
        CapturingClosureRootCase {
            name: "capturing_closure_match_guard_bound_control_flow_build",
            source_relative: "fixtures/codegen/pass/capturing_closure_match_guard_bound_control_flow_build.ql",
            context: "capturing closure match-guard bound control-flow build regression",
        },
        CapturingClosureRootCase {
            name: "capturing_closure_match_guard_block_assignment_bound_build",
            source_relative: "fixtures/codegen/pass/capturing_closure_match_guard_block_assignment_bound_build.ql",
            context: "capturing closure match-guard block assignment-bound build regression",
        },
        CapturingClosureRootCase {
            name: "capturing_closure_match_guard_different_closure_block_alias_build",
            source_relative: "fixtures/codegen/pass/capturing_closure_match_guard_different_closure_block_alias_build.ql",
            context: "capturing closure match-guard different-closure block-alias build regression",
        },
        CapturingClosureRootCase {
            name: "capturing_closure_match_guard_different_closure_block_binding_build",
            source_relative: "fixtures/codegen/pass/capturing_closure_match_guard_different_closure_block_binding_build.ql",
            context: "capturing closure match-guard different-closure block-binding build regression",
        },
    ]
}
