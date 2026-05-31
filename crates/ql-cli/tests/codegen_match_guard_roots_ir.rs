mod support;

use support::codegen::{PassCase, run_pass_case};
use support::workspace_root;

struct MatchGuardRootIrCase {
    name: &'static str,
    source_relative: &'static str,
    expected_relative: &'static str,
    context: &'static str,
}

#[test]
fn match_guard_root_ir_codegen_cases_match() {
    let workspace_root = workspace_root();
    let mut failures = Vec::new();

    for case in match_guard_root_ir_cases() {
        let pass_case = PassCase {
            name: case.name,
            source_relative: case.source_relative,
            emit: "llvm-ir",
            expected_relative: case.expected_relative,
            mock_compiler: false,
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
        "match/guard root LLVM IR codegen regressions:\n\n{}",
        failures.join("\n\n")
    );
}

fn match_guard_root_ir_cases() -> &'static [MatchGuardRootIrCase] {
    &[
        MatchGuardRootIrCase {
            name: "match_guard_aggregate_call_args_llvm_ir",
            source_relative: "fixtures/codegen/pass/match_guard_aggregate_call_args.ql",
            expected_relative: "tests/codegen/pass/match_guard_aggregate_call_args.ll",
            context: "match guard aggregate call args LLVM IR regression",
        },
        MatchGuardRootIrCase {
            name: "match_guard_inline_aggregate_call_args_llvm_ir",
            source_relative: "fixtures/codegen/pass/match_guard_inline_aggregate_call_args.ql",
            expected_relative: "tests/codegen/pass/match_guard_inline_aggregate_call_args.ll",
            context: "match guard inline aggregate call args LLVM IR regression",
        },
        MatchGuardRootIrCase {
            name: "match_guard_inline_projection_roots_llvm_ir",
            source_relative: "fixtures/codegen/pass/match_guard_inline_projection_roots.ql",
            expected_relative: "tests/codegen/pass/match_guard_inline_projection_roots.ll",
            context: "match guard inline projection roots LLVM IR regression",
        },
        MatchGuardRootIrCase {
            name: "match_guard_item_backed_inline_combos_llvm_ir",
            source_relative: "fixtures/codegen/pass/match_guard_item_backed_inline_combos.ql",
            expected_relative: "tests/codegen/pass/match_guard_item_backed_inline_combos.ll",
            context: "match guard item-backed inline combos LLVM IR regression",
        },
        MatchGuardRootIrCase {
            name: "match_guard_call_backed_combos_llvm_ir",
            source_relative: "fixtures/codegen/pass/match_guard_call_backed_combos.ql",
            expected_relative: "tests/codegen/pass/match_guard_call_backed_combos.ll",
            context: "match guard call-backed combos LLVM IR regression",
        },
        MatchGuardRootIrCase {
            name: "match_guard_call_root_nested_runtime_projection_llvm_ir",
            source_relative: "fixtures/codegen/pass/match_guard_call_root_nested_runtime_projection.ql",
            expected_relative: "tests/codegen/pass/match_guard_call_root_nested_runtime_projection.ll",
            context: "match guard call-root nested runtime projection LLVM IR regression",
        },
        MatchGuardRootIrCase {
            name: "match_guard_nested_call_root_inline_combos_llvm_ir",
            source_relative: "fixtures/codegen/pass/match_guard_nested_call_root_inline_combos.ql",
            expected_relative: "tests/codegen/pass/match_guard_nested_call_root_inline_combos.ll",
            context: "match guard nested call-root inline combos LLVM IR regression",
        },
        MatchGuardRootIrCase {
            name: "match_guard_item_backed_nested_call_root_combos_llvm_ir",
            source_relative: "fixtures/codegen/pass/match_guard_item_backed_nested_call_root_combos.ql",
            expected_relative: "tests/codegen/pass/match_guard_item_backed_nested_call_root_combos.ll",
            context: "match guard item-backed nested call-root combos LLVM IR regression",
        },
        MatchGuardRootIrCase {
            name: "match_guard_call_backed_nested_call_root_combos_llvm_ir",
            source_relative: "fixtures/codegen/pass/match_guard_call_backed_nested_call_root_combos.ql",
            expected_relative: "tests/codegen/pass/match_guard_call_backed_nested_call_root_combos.ll",
            context: "match guard call-backed nested call-root combos LLVM IR regression",
        },
        MatchGuardRootIrCase {
            name: "match_guard_alias_backed_nested_call_root_combos_llvm_ir",
            source_relative: "fixtures/codegen/pass/match_guard_alias_backed_nested_call_root_combos.ql",
            expected_relative: "tests/codegen/pass/match_guard_alias_backed_nested_call_root_combos.ll",
            context: "match guard alias-backed nested call-root combos LLVM IR regression",
        },
        MatchGuardRootIrCase {
            name: "match_guard_binding_backed_nested_call_root_combos_llvm_ir",
            source_relative: "fixtures/codegen/pass/match_guard_binding_backed_nested_call_root_combos.ql",
            expected_relative: "tests/codegen/pass/match_guard_binding_backed_nested_call_root_combos.ll",
            context: "match guard binding-backed nested call-root combos LLVM IR regression",
        },
        MatchGuardRootIrCase {
            name: "match_guard_projection_backed_nested_call_root_combos_llvm_ir",
            source_relative: "fixtures/codegen/pass/match_guard_projection_backed_nested_call_root_combos.ql",
            expected_relative: "tests/codegen/pass/match_guard_projection_backed_nested_call_root_combos.ll",
            context: "match guard projection-backed nested call-root combos LLVM IR regression",
        },
    ]
}
