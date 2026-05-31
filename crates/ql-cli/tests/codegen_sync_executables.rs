mod support;

use support::codegen::{PassCase, run_pass_case};
use support::workspace_root;

#[test]
fn sync_executable_codegen_cases_match() {
    let workspace_root = workspace_root();
    let mut failures = Vec::new();

    for stem in sync_executable_stems() {
        let source_relative = format!("fixtures/codegen/pass/{stem}.ql");
        let case_name = format!("{stem}_exe");
        let pass_case = PassCase {
            name: &case_name,
            source_relative: &source_relative,
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        };

        if let Err(message) = run_pass_case(&workspace_root, &pass_case) {
            failures.push(format!("{} executable regression:\n\n{}", stem, message));
        }
    }

    assert!(
        failures.is_empty(),
        "sync executable codegen regressions:\n\n{}",
        failures.join("\n\n")
    );
}

fn sync_executable_stems() -> &'static [&'static str] {
    &[
        "for_array",
        "bool_match",
        "integer_match",
        "integer_dynamic_guard_match",
        "integer_comparison_guard_match",
        "projected_integer_comparison_guard_match",
        "const_projected_integer_comparison_guard_match",
        "integer_dynamic_guard_catch_all_match",
        "integer_match_binding",
        "literal_guard_match",
        "const_guard_match",
        "bool_dynamic_guard_match",
        "negated_bool_guard_match",
        "alias_const_guard_match",
        "match_guard_direct_calls",
        "match_guard_call_projection_roots",
        "match_guard_aggregate_call_args",
        "match_guard_inline_aggregate_call_args",
        "match_guard_inline_projection_roots",
        "match_guard_item_backed_inline_combos",
        "match_guard_call_backed_combos",
        "match_guard_call_root_nested_runtime_projection",
        "match_guard_nested_call_root_inline_combos",
        "match_guard_item_backed_nested_call_root_combos",
        "match_guard_call_backed_nested_call_root_combos",
        "match_guard_alias_backed_nested_call_root_combos",
        "match_guard_binding_backed_nested_call_root_combos",
        "match_guard_projection_backed_nested_call_root_combos",
        "for_call_root_fixed_shapes",
        "import_alias_call_root_fixed_shapes",
        "nested_call_root_fixed_shapes",
        "import_alias_nested_call_root_fixed_shapes",
    ]
}
