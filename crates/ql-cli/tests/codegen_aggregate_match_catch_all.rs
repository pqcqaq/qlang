mod support;

use support::codegen::{PassCase, run_pass_case};
use support::workspace_root;

struct AggregateCatchAllCase {
    name: &'static str,
    source_relative: &'static str,
    context: &'static str,
}

#[test]
fn aggregate_match_catch_all_codegen_cases_match() {
    let workspace_root = workspace_root();
    let mut failures = Vec::new();

    for case in aggregate_match_catch_all_cases() {
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
        "aggregate match catch-all codegen regressions:\n\n{}",
        failures.join("\n\n")
    );
}

fn aggregate_match_catch_all_cases() -> &'static [AggregateCatchAllCase] {
    &[
        AggregateCatchAllCase {
            name: "fixed_array_match_catch_all_build",
            source_relative: "fixtures/codegen/pass/fixed_array_match_catch_all.ql",
            context: "fixed-array match catch-all build regression",
        },
        AggregateCatchAllCase {
            name: "tuple_struct_match_catch_all_build",
            source_relative: "fixtures/codegen/pass/tuple_struct_match_catch_all.ql",
            context: "tuple/struct match catch-all build regression",
        },
        AggregateCatchAllCase {
            name: "projected_aggregate_match_catch_all_build",
            source_relative: "fixtures/codegen/pass/projected_aggregate_match_catch_all.ql",
            context: "projected aggregate match catch-all build regression",
        },
        AggregateCatchAllCase {
            name: "import_alias_projected_aggregate_match_catch_all_build",
            source_relative: "fixtures/codegen/pass/import_alias_projected_aggregate_match_catch_all.ql",
            context: "import-alias projected aggregate match catch-all build regression",
        },
        AggregateCatchAllCase {
            name: "import_alias_control_flow_projected_aggregate_match_catch_all_build",
            source_relative: "fixtures/codegen/pass/import_alias_control_flow_projected_aggregate_match_catch_all.ql",
            context: "import-alias control-flow projected aggregate match catch-all build regression",
        },
        AggregateCatchAllCase {
            name: "control_flow_projected_aggregate_match_catch_all_build",
            source_relative: "fixtures/codegen/pass/control_flow_projected_aggregate_match_catch_all.ql",
            context: "control-flow projected aggregate match catch-all build regression",
        },
        AggregateCatchAllCase {
            name: "nested_projected_aggregate_match_catch_all_build",
            source_relative: "fixtures/codegen/pass/nested_projected_aggregate_match_catch_all.ql",
            context: "nested projected aggregate match catch-all build regression",
        },
        AggregateCatchAllCase {
            name: "import_alias_nested_projected_aggregate_match_catch_all_build",
            source_relative: "fixtures/codegen/pass/import_alias_nested_projected_aggregate_match_catch_all.ql",
            context: "import-alias nested projected aggregate match catch-all build regression",
        },
        AggregateCatchAllCase {
            name: "control_flow_nested_projected_aggregate_match_catch_all_build",
            source_relative: "fixtures/codegen/pass/control_flow_nested_projected_aggregate_match_catch_all.ql",
            context: "control-flow nested projected aggregate match catch-all build regression",
        },
        AggregateCatchAllCase {
            name: "import_alias_control_flow_nested_projected_aggregate_match_catch_all_build",
            source_relative: "fixtures/codegen/pass/import_alias_control_flow_nested_projected_aggregate_match_catch_all.ql",
            context: "import-alias control-flow nested projected aggregate match catch-all build regression",
        },
        AggregateCatchAllCase {
            name: "question_wrapped_projected_aggregate_match_catch_all_build",
            source_relative: "fixtures/codegen/pass/question_wrapped_projected_aggregate_match_catch_all.ql",
            context: "question-wrapped projected aggregate match catch-all build regression",
        },
        AggregateCatchAllCase {
            name: "import_alias_question_wrapped_projected_aggregate_match_catch_all_build",
            source_relative: "fixtures/codegen/pass/import_alias_question_wrapped_projected_aggregate_match_catch_all.ql",
            context: "import-alias question-wrapped projected aggregate match catch-all build regression",
        },
        AggregateCatchAllCase {
            name: "control_flow_question_wrapped_projected_aggregate_match_catch_all_build",
            source_relative: "fixtures/codegen/pass/control_flow_question_wrapped_projected_aggregate_match_catch_all.ql",
            context: "control-flow question-wrapped projected aggregate match catch-all build regression",
        },
        AggregateCatchAllCase {
            name: "import_alias_control_flow_question_wrapped_projected_aggregate_match_catch_all_build",
            source_relative: "fixtures/codegen/pass/import_alias_control_flow_question_wrapped_projected_aggregate_match_catch_all.ql",
            context: "import-alias control-flow question-wrapped projected aggregate match catch-all build regression",
        },
        AggregateCatchAllCase {
            name: "question_wrapped_nested_projected_aggregate_match_catch_all_build",
            source_relative: "fixtures/codegen/pass/question_wrapped_nested_projected_aggregate_match_catch_all.ql",
            context: "question-wrapped nested projected aggregate match catch-all build regression",
        },
        AggregateCatchAllCase {
            name: "import_alias_question_wrapped_nested_projected_aggregate_match_catch_all_build",
            source_relative: "fixtures/codegen/pass/import_alias_question_wrapped_nested_projected_aggregate_match_catch_all.ql",
            context: "import-alias question-wrapped nested projected aggregate match catch-all build regression",
        },
        AggregateCatchAllCase {
            name: "control_flow_question_wrapped_nested_projected_aggregate_match_catch_all_build",
            source_relative: "fixtures/codegen/pass/control_flow_question_wrapped_nested_projected_aggregate_match_catch_all.ql",
            context: "control-flow question-wrapped nested projected aggregate match catch-all build regression",
        },
        AggregateCatchAllCase {
            name: "import_alias_control_flow_question_wrapped_nested_projected_aggregate_match_catch_all_build",
            source_relative: "fixtures/codegen/pass/import_alias_control_flow_question_wrapped_nested_projected_aggregate_match_catch_all.ql",
            context: "import-alias control-flow question-wrapped nested projected aggregate match catch-all build regression",
        },
    ]
}
