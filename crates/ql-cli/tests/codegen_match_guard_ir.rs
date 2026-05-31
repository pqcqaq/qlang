mod support;

use support::codegen::{PassCase, run_pass_case};
use support::workspace_root;

struct MatchGuardIrCase {
    name: &'static str,
    source_relative: &'static str,
    expected_relative: &'static str,
    context: &'static str,
}

#[test]
fn match_guard_ir_codegen_cases_match() {
    let workspace_root = workspace_root();
    let mut failures = Vec::new();

    for case in match_guard_ir_cases() {
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
        "match/guard LLVM IR codegen regressions:\n\n{}",
        failures.join("\n\n")
    );
}

fn match_guard_ir_cases() -> &'static [MatchGuardIrCase] {
    &[
        MatchGuardIrCase {
            name: "bool_match_llvm_ir",
            source_relative: "fixtures/codegen/pass/bool_match.ql",
            expected_relative: "tests/codegen/pass/bool_match.ll",
            context: "bool match LLVM IR regression",
        },
        MatchGuardIrCase {
            name: "integer_match_llvm_ir",
            source_relative: "fixtures/codegen/pass/integer_match.ql",
            expected_relative: "tests/codegen/pass/integer_match.ll",
            context: "integer match LLVM IR regression",
        },
        MatchGuardIrCase {
            name: "integer_dynamic_guard_match_llvm_ir",
            source_relative: "fixtures/codegen/pass/integer_dynamic_guard_match.ql",
            expected_relative: "tests/codegen/pass/integer_dynamic_guard_match.ll",
            context: "integer dynamic guard match LLVM IR regression",
        },
        MatchGuardIrCase {
            name: "integer_comparison_guard_match_llvm_ir",
            source_relative: "fixtures/codegen/pass/integer_comparison_guard_match.ql",
            expected_relative: "tests/codegen/pass/integer_comparison_guard_match.ll",
            context: "integer comparison guard match LLVM IR regression",
        },
        MatchGuardIrCase {
            name: "projected_integer_comparison_guard_match_llvm_ir",
            source_relative: "fixtures/codegen/pass/projected_integer_comparison_guard_match.ql",
            expected_relative: "tests/codegen/pass/projected_integer_comparison_guard_match.ll",
            context: "projected integer comparison guard match LLVM IR regression",
        },
        MatchGuardIrCase {
            name: "const_projected_integer_comparison_guard_match_llvm_ir",
            source_relative: "fixtures/codegen/pass/const_projected_integer_comparison_guard_match.ql",
            expected_relative: "tests/codegen/pass/const_projected_integer_comparison_guard_match.ll",
            context: "const projected integer comparison guard match LLVM IR regression",
        },
        MatchGuardIrCase {
            name: "integer_dynamic_guard_catch_all_match_llvm_ir",
            source_relative: "fixtures/codegen/pass/integer_dynamic_guard_catch_all_match.ql",
            expected_relative: "tests/codegen/pass/integer_dynamic_guard_catch_all_match.ll",
            context: "integer dynamic guard catch-all match LLVM IR regression",
        },
        MatchGuardIrCase {
            name: "integer_match_binding_llvm_ir",
            source_relative: "fixtures/codegen/pass/integer_match_binding.ql",
            expected_relative: "tests/codegen/pass/integer_match_binding.ll",
            context: "integer match binding LLVM IR regression",
        },
        MatchGuardIrCase {
            name: "literal_guard_match_llvm_ir",
            source_relative: "fixtures/codegen/pass/literal_guard_match.ql",
            expected_relative: "tests/codegen/pass/literal_guard_match.ll",
            context: "literal guard match LLVM IR regression",
        },
        MatchGuardIrCase {
            name: "const_guard_match_llvm_ir",
            source_relative: "fixtures/codegen/pass/const_guard_match.ql",
            expected_relative: "tests/codegen/pass/const_guard_match.ll",
            context: "const guard match LLVM IR regression",
        },
        MatchGuardIrCase {
            name: "bool_dynamic_guard_match_llvm_ir",
            source_relative: "fixtures/codegen/pass/bool_dynamic_guard_match.ql",
            expected_relative: "tests/codegen/pass/bool_dynamic_guard_match.ll",
            context: "bool dynamic guard match LLVM IR regression",
        },
        MatchGuardIrCase {
            name: "negated_bool_guard_match_llvm_ir",
            source_relative: "fixtures/codegen/pass/negated_bool_guard_match.ql",
            expected_relative: "tests/codegen/pass/negated_bool_guard_match.ll",
            context: "negated bool guard match LLVM IR regression",
        },
        MatchGuardIrCase {
            name: "bool_short_circuit_expr_llvm_ir",
            source_relative: "fixtures/codegen/pass/bool_short_circuit_expr.ql",
            expected_relative: "tests/codegen/pass/bool_short_circuit_expr.ll",
            context: "bool short-circuit expression LLVM IR regression",
        },
        MatchGuardIrCase {
            name: "alias_const_guard_match_llvm_ir",
            source_relative: "fixtures/codegen/pass/alias_const_guard_match.ql",
            expected_relative: "tests/codegen/pass/alias_const_guard_match.ll",
            context: "alias const guard match LLVM IR regression",
        },
        MatchGuardIrCase {
            name: "match_guard_direct_calls_llvm_ir",
            source_relative: "fixtures/codegen/pass/match_guard_direct_calls.ql",
            expected_relative: "tests/codegen/pass/match_guard_direct_calls.ll",
            context: "match guard direct call LLVM IR regression",
        },
        MatchGuardIrCase {
            name: "match_guard_call_projection_roots_llvm_ir",
            source_relative: "fixtures/codegen/pass/match_guard_call_projection_roots.ql",
            expected_relative: "tests/codegen/pass/match_guard_call_projection_roots.ll",
            context: "match guard call projection root LLVM IR regression",
        },
    ]
}
