mod support;

use support::codegen::{PassCase, run_pass_case};
use support::workspace_root;

struct CleanupCapturingClosureCase {
    name: &'static str,
    source_relative: &'static str,
    context: &'static str,
}

#[test]
fn cleanup_capturing_closure_codegen_cases_match() {
    let workspace_root = workspace_root();
    let mut failures = Vec::new();

    for case in cleanup_capturing_closure_cases() {
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
        "cleanup capturing closure codegen regressions:\n\n{}",
        failures.join("\n\n")
    );
}

fn cleanup_capturing_closure_cases() -> &'static [CleanupCapturingClosureCase] {
    &[
        CleanupCapturingClosureCase {
            name: "cleanup_block_capturing_closure_alias_build",
            source_relative: "fixtures/codegen/pass/cleanup_block_capturing_closure_alias_build.ql",
            context: "cleanup block capturing-closure alias build regression",
        },
        CleanupCapturingClosureCase {
            name: "cleanup_block_mutable_capturing_closure_reassign_build",
            source_relative: "fixtures/codegen/pass/cleanup_block_mutable_capturing_closure_reassign_build.ql",
            context: "cleanup block mutable capturing-closure reassign build regression",
        },
        CleanupCapturingClosureCase {
            name: "cleanup_assignment_valued_capturing_closure_build",
            source_relative: "fixtures/codegen/pass/cleanup_assignment_valued_capturing_closure_build.ql",
            context: "cleanup assignment-valued capturing-closure build regression",
        },
        CleanupCapturingClosureCase {
            name: "cleanup_control_flow_assignment_valued_capturing_closure_build",
            source_relative: "fixtures/codegen/pass/cleanup_control_flow_assignment_valued_capturing_closure_build.ql",
            context: "cleanup control-flow assignment-valued capturing-closure build regression",
        },
        CleanupCapturingClosureCase {
            name: "cleanup_if_shared_local_control_flow_capturing_closure_alias_chain_build",
            source_relative: "fixtures/codegen/pass/cleanup_if_shared_local_control_flow_capturing_closure_alias_chain_build.ql",
            context: "cleanup if shared-local control-flow capturing-closure alias-chain build regression",
        },
        CleanupCapturingClosureCase {
            name: "cleanup_match_shared_local_control_flow_capturing_closure_alias_chain_build",
            source_relative: "fixtures/codegen/pass/cleanup_match_shared_local_control_flow_capturing_closure_alias_chain_build.ql",
            context: "cleanup match shared-local control-flow capturing-closure alias-chain build regression",
        },
        CleanupCapturingClosureCase {
            name: "cleanup_guarded_match_shared_local_control_flow_capturing_closure_alias_chain_build",
            source_relative: "fixtures/codegen/pass/cleanup_guarded_match_shared_local_control_flow_capturing_closure_alias_chain_build.ql",
            context: "cleanup guarded match shared-local control-flow capturing-closure alias-chain build regression",
        },
        CleanupCapturingClosureCase {
            name: "cleanup_block_assignment_valued_capturing_closure_build",
            source_relative: "fixtures/codegen/pass/cleanup_block_assignment_valued_capturing_closure_build.ql",
            context: "cleanup block assignment-valued capturing-closure build regression",
        },
        CleanupCapturingClosureCase {
            name: "cleanup_block_control_flow_assignment_valued_capturing_closure_build",
            source_relative: "fixtures/codegen/pass/cleanup_block_control_flow_assignment_valued_capturing_closure_build.ql",
            context: "cleanup block control-flow assignment-valued capturing-closure build regression",
        },
        CleanupCapturingClosureCase {
            name: "cleanup_block_control_flow_local_alias_capturing_closure_build",
            source_relative: "fixtures/codegen/pass/cleanup_block_control_flow_local_alias_capturing_closure_build.ql",
            context: "cleanup block control-flow local-alias capturing-closure build regression",
        },
        CleanupCapturingClosureCase {
            name: "cleanup_control_flow_local_alias_capturing_closure_call_roots_build",
            source_relative: "fixtures/codegen/pass/cleanup_control_flow_local_alias_capturing_closure_call_roots_build.ql",
            context: "cleanup control-flow local-alias capturing-closure call-root build regression",
        },
    ]
}
