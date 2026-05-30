mod support;

use support::codegen::{PassCase, run_pass_case};
use support::workspace_root;

struct CleanupBlockCase {
    name: &'static str,
    source_relative: &'static str,
    context: &'static str,
}

#[test]
fn cleanup_block_basic_codegen_cases_match() {
    let workspace_root = workspace_root();
    let mut failures = Vec::new();

    for case in cleanup_block_cases() {
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
        "cleanup block basic codegen regressions:\n\n{}",
        failures.join("\n\n")
    );
}

fn cleanup_block_cases() -> &'static [CleanupBlockCase] {
    &[
        CleanupBlockCase {
            name: "cleanup_block_sequence_build",
            source_relative: "fixtures/codegen/pass/cleanup_block_sequence.ql",
            context: "cleanup block sequence build regression",
        },
        CleanupBlockCase {
            name: "cleanup_block_let_binding_build",
            source_relative: "fixtures/codegen/pass/cleanup_block_let_binding.ql",
            context: "cleanup block let binding build regression",
        },
        CleanupBlockCase {
            name: "cleanup_block_let_destructuring_build",
            source_relative: "fixtures/codegen/pass/cleanup_block_let_destructuring.ql",
            context: "cleanup block let destructuring build regression",
        },
        CleanupBlockCase {
            name: "cleanup_fixed_array_destructuring_build",
            source_relative: "fixtures/codegen/pass/cleanup_fixed_array_destructuring.ql",
            context: "cleanup fixed-array destructuring build regression",
        },
        CleanupBlockCase {
            name: "cleanup_block_while_build",
            source_relative: "fixtures/codegen/pass/cleanup_block_while.ql",
            context: "cleanup block while build regression",
        },
        CleanupBlockCase {
            name: "cleanup_block_while_break_continue_build",
            source_relative: "fixtures/codegen/pass/cleanup_block_while_break_continue.ql",
            context: "cleanup block while break/continue build regression",
        },
        CleanupBlockCase {
            name: "cleanup_block_loop_break_continue_build",
            source_relative: "fixtures/codegen/pass/cleanup_block_loop_break_continue.ql",
            context: "cleanup block loop break/continue build regression",
        },
    ]
}
