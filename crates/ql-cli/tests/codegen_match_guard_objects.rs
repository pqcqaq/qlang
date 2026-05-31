mod support;

use support::codegen::{PassCase, run_pass_case};
use support::workspace_root;

struct MatchGuardObjectCase {
    name: &'static str,
    source_relative: &'static str,
    context: &'static str,
}

#[test]
fn match_guard_object_codegen_cases_match() {
    let workspace_root = workspace_root();
    let mut failures = Vec::new();

    for case in match_guard_object_cases() {
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
        "match/guard object codegen regressions:\n\n{}",
        failures.join("\n\n")
    );
}

fn match_guard_object_cases() -> &'static [MatchGuardObjectCase] {
    &[
        MatchGuardObjectCase {
            name: "minimal_build_object",
            source_relative: "fixtures/codegen/pass/minimal_build.ql",
            context: "minimal build object regression",
        },
        MatchGuardObjectCase {
            name: "bool_match_object",
            source_relative: "fixtures/codegen/pass/bool_match.ql",
            context: "bool match object regression",
        },
        MatchGuardObjectCase {
            name: "integer_match_object",
            source_relative: "fixtures/codegen/pass/integer_match.ql",
            context: "integer match object regression",
        },
        MatchGuardObjectCase {
            name: "integer_dynamic_guard_match_object",
            source_relative: "fixtures/codegen/pass/integer_dynamic_guard_match.ql",
            context: "integer dynamic guard match object regression",
        },
        MatchGuardObjectCase {
            name: "integer_comparison_guard_match_object",
            source_relative: "fixtures/codegen/pass/integer_comparison_guard_match.ql",
            context: "integer comparison guard match object regression",
        },
        MatchGuardObjectCase {
            name: "projected_integer_comparison_guard_match_object",
            source_relative: "fixtures/codegen/pass/projected_integer_comparison_guard_match.ql",
            context: "projected integer comparison guard match object regression",
        },
        MatchGuardObjectCase {
            name: "const_projected_integer_comparison_guard_match_object",
            source_relative: "fixtures/codegen/pass/const_projected_integer_comparison_guard_match.ql",
            context: "const projected integer comparison guard match object regression",
        },
        MatchGuardObjectCase {
            name: "integer_dynamic_guard_catch_all_match_object",
            source_relative: "fixtures/codegen/pass/integer_dynamic_guard_catch_all_match.ql",
            context: "integer dynamic guard catch-all match object regression",
        },
        MatchGuardObjectCase {
            name: "integer_match_binding_object",
            source_relative: "fixtures/codegen/pass/integer_match_binding.ql",
            context: "integer match binding object regression",
        },
        MatchGuardObjectCase {
            name: "literal_guard_match_object",
            source_relative: "fixtures/codegen/pass/literal_guard_match.ql",
            context: "literal guard match object regression",
        },
        MatchGuardObjectCase {
            name: "const_guard_match_object",
            source_relative: "fixtures/codegen/pass/const_guard_match.ql",
            context: "const guard match object regression",
        },
        MatchGuardObjectCase {
            name: "bool_dynamic_guard_match_object",
            source_relative: "fixtures/codegen/pass/bool_dynamic_guard_match.ql",
            context: "bool dynamic guard match object regression",
        },
        MatchGuardObjectCase {
            name: "negated_bool_guard_match_object",
            source_relative: "fixtures/codegen/pass/negated_bool_guard_match.ql",
            context: "negated bool guard match object regression",
        },
        MatchGuardObjectCase {
            name: "alias_const_guard_match_object",
            source_relative: "fixtures/codegen/pass/alias_const_guard_match.ql",
            context: "alias const guard match object regression",
        },
        MatchGuardObjectCase {
            name: "match_guard_direct_calls_object",
            source_relative: "fixtures/codegen/pass/match_guard_direct_calls.ql",
            context: "match guard direct calls object regression",
        },
        MatchGuardObjectCase {
            name: "match_guard_call_projection_roots_object",
            source_relative: "fixtures/codegen/pass/match_guard_call_projection_roots.ql",
            context: "match guard call projection roots object regression",
        },
        MatchGuardObjectCase {
            name: "match_guard_aggregate_call_args_object",
            source_relative: "fixtures/codegen/pass/match_guard_aggregate_call_args.ql",
            context: "match guard aggregate call args object regression",
        },
        MatchGuardObjectCase {
            name: "match_guard_inline_aggregate_call_args_object",
            source_relative: "fixtures/codegen/pass/match_guard_inline_aggregate_call_args.ql",
            context: "match guard inline aggregate call args object regression",
        },
        MatchGuardObjectCase {
            name: "match_guard_inline_projection_roots_object",
            source_relative: "fixtures/codegen/pass/match_guard_inline_projection_roots.ql",
            context: "match guard inline projection roots object regression",
        },
        MatchGuardObjectCase {
            name: "match_guard_item_backed_inline_combos_object",
            source_relative: "fixtures/codegen/pass/match_guard_item_backed_inline_combos.ql",
            context: "match guard item-backed inline combos object regression",
        },
        MatchGuardObjectCase {
            name: "match_guard_call_backed_combos_object",
            source_relative: "fixtures/codegen/pass/match_guard_call_backed_combos.ql",
            context: "match guard call-backed combos object regression",
        },
        MatchGuardObjectCase {
            name: "match_guard_call_root_nested_runtime_projection_object",
            source_relative: "fixtures/codegen/pass/match_guard_call_root_nested_runtime_projection.ql",
            context: "match guard call-root nested runtime projection object regression",
        },
        MatchGuardObjectCase {
            name: "match_guard_nested_call_root_inline_combos_object",
            source_relative: "fixtures/codegen/pass/match_guard_nested_call_root_inline_combos.ql",
            context: "match guard nested call-root inline combos object regression",
        },
        MatchGuardObjectCase {
            name: "match_guard_item_backed_nested_call_root_combos_object",
            source_relative: "fixtures/codegen/pass/match_guard_item_backed_nested_call_root_combos.ql",
            context: "match guard item-backed nested call-root combos object regression",
        },
        MatchGuardObjectCase {
            name: "match_guard_call_backed_nested_call_root_combos_object",
            source_relative: "fixtures/codegen/pass/match_guard_call_backed_nested_call_root_combos.ql",
            context: "match guard call-backed nested call-root combos object regression",
        },
        MatchGuardObjectCase {
            name: "match_guard_alias_backed_nested_call_root_combos_object",
            source_relative: "fixtures/codegen/pass/match_guard_alias_backed_nested_call_root_combos.ql",
            context: "match guard alias-backed nested call-root combos object regression",
        },
        MatchGuardObjectCase {
            name: "match_guard_binding_backed_nested_call_root_combos_object",
            source_relative: "fixtures/codegen/pass/match_guard_binding_backed_nested_call_root_combos.ql",
            context: "match guard binding-backed nested call-root combos object regression",
        },
        MatchGuardObjectCase {
            name: "match_guard_projection_backed_nested_call_root_combos_object",
            source_relative: "fixtures/codegen/pass/match_guard_projection_backed_nested_call_root_combos.ql",
            context: "match guard projection-backed nested call-root combos object regression",
        },
        MatchGuardObjectCase {
            name: "for_call_root_fixed_shapes_object",
            source_relative: "fixtures/codegen/pass/for_call_root_fixed_shapes.ql",
            context: "for call-root fixed-shapes object regression",
        },
        MatchGuardObjectCase {
            name: "import_alias_call_root_fixed_shapes_object",
            source_relative: "fixtures/codegen/pass/import_alias_call_root_fixed_shapes.ql",
            context: "import alias call-root fixed-shapes object regression",
        },
        MatchGuardObjectCase {
            name: "nested_call_root_fixed_shapes_object",
            source_relative: "fixtures/codegen/pass/nested_call_root_fixed_shapes.ql",
            context: "nested call-root fixed-shapes object regression",
        },
        MatchGuardObjectCase {
            name: "import_alias_nested_call_root_fixed_shapes_object",
            source_relative: "fixtures/codegen/pass/import_alias_nested_call_root_fixed_shapes.ql",
            context: "import alias nested call-root fixed-shapes object regression",
        },
    ]
}
