mod support;

use support::codegen::{PassCase, run_pass_case};
use support::workspace_root;

struct CallRootCatchAllCase {
    name: &'static str,
    source_relative: &'static str,
    context: &'static str,
}

#[test]
fn call_root_match_catch_all_codegen_cases_match() {
    let workspace_root = workspace_root();
    let mut failures = Vec::new();

    for case in call_root_match_catch_all_cases() {
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
        "call-root match catch-all codegen regressions:\n\n{}",
        failures.join("\n\n")
    );
}

fn call_root_match_catch_all_cases() -> &'static [CallRootCatchAllCase] {
    &[
        CallRootCatchAllCase {
            name: "call_root_aggregate_match_catch_all_build",
            source_relative: "fixtures/codegen/pass/call_root_aggregate_match_catch_all.ql",
            context: "call-root aggregate match catch-all build regression",
        },
        CallRootCatchAllCase {
            name: "import_alias_call_root_aggregate_match_catch_all_build",
            source_relative: "fixtures/codegen/pass/import_alias_call_root_aggregate_match_catch_all.ql",
            context: "import-alias call-root aggregate match catch-all build regression",
        },
        CallRootCatchAllCase {
            name: "control_flow_call_root_aggregate_match_catch_all_build",
            source_relative: "fixtures/codegen/pass/control_flow_call_root_aggregate_match_catch_all.ql",
            context: "control-flow call-root aggregate match catch-all build regression",
        },
        CallRootCatchAllCase {
            name: "import_alias_control_flow_call_root_aggregate_match_catch_all_build",
            source_relative: "fixtures/codegen/pass/import_alias_control_flow_call_root_aggregate_match_catch_all.ql",
            context: "import-alias control-flow call-root aggregate match catch-all build regression",
        },
        CallRootCatchAllCase {
            name: "question_wrapped_call_root_aggregate_match_catch_all_build",
            source_relative: "fixtures/codegen/pass/question_wrapped_call_root_aggregate_match_catch_all.ql",
            context: "question-wrapped call-root aggregate match catch-all build regression",
        },
        CallRootCatchAllCase {
            name: "import_alias_question_wrapped_call_root_aggregate_match_catch_all_build",
            source_relative: "fixtures/codegen/pass/import_alias_question_wrapped_call_root_aggregate_match_catch_all.ql",
            context: "import-alias question-wrapped call-root aggregate match catch-all build regression",
        },
        CallRootCatchAllCase {
            name: "control_flow_question_wrapped_call_root_aggregate_match_catch_all_build",
            source_relative: "fixtures/codegen/pass/control_flow_question_wrapped_call_root_aggregate_match_catch_all.ql",
            context: "control-flow question-wrapped call-root aggregate match catch-all build regression",
        },
        CallRootCatchAllCase {
            name: "import_alias_control_flow_question_wrapped_call_root_aggregate_match_catch_all_build",
            source_relative: "fixtures/codegen/pass/import_alias_control_flow_question_wrapped_call_root_aggregate_match_catch_all.ql",
            context: "import-alias control-flow question-wrapped call-root aggregate match catch-all build regression",
        },
        CallRootCatchAllCase {
            name: "question_wrapped_nested_call_root_aggregate_match_catch_all_build",
            source_relative: "fixtures/codegen/pass/question_wrapped_nested_call_root_aggregate_match_catch_all.ql",
            context: "question-wrapped nested call-root aggregate match catch-all build regression",
        },
        CallRootCatchAllCase {
            name: "import_alias_question_wrapped_nested_call_root_aggregate_match_catch_all_build",
            source_relative: "fixtures/codegen/pass/import_alias_question_wrapped_nested_call_root_aggregate_match_catch_all.ql",
            context: "import-alias question-wrapped nested call-root aggregate match catch-all build regression",
        },
        CallRootCatchAllCase {
            name: "control_flow_question_wrapped_nested_call_root_aggregate_match_catch_all_build",
            source_relative: "fixtures/codegen/pass/control_flow_question_wrapped_nested_call_root_aggregate_match_catch_all.ql",
            context: "control-flow question-wrapped nested call-root aggregate match catch-all build regression",
        },
        CallRootCatchAllCase {
            name: "import_alias_control_flow_question_wrapped_nested_call_root_aggregate_match_catch_all_build",
            source_relative: "fixtures/codegen/pass/import_alias_control_flow_question_wrapped_nested_call_root_aggregate_match_catch_all.ql",
            context: "import-alias control-flow question-wrapped nested call-root aggregate match catch-all build regression",
        },
        CallRootCatchAllCase {
            name: "nested_call_root_aggregate_match_catch_all_build",
            source_relative: "fixtures/codegen/pass/nested_call_root_aggregate_match_catch_all.ql",
            context: "nested call-root aggregate match catch-all build regression",
        },
        CallRootCatchAllCase {
            name: "import_alias_nested_call_root_aggregate_match_catch_all_build",
            source_relative: "fixtures/codegen/pass/import_alias_nested_call_root_aggregate_match_catch_all.ql",
            context: "import-alias nested call-root aggregate match catch-all build regression",
        },
        CallRootCatchAllCase {
            name: "control_flow_nested_call_root_aggregate_match_catch_all_build",
            source_relative: "fixtures/codegen/pass/control_flow_nested_call_root_aggregate_match_catch_all.ql",
            context: "control-flow nested call-root aggregate match catch-all build regression",
        },
        CallRootCatchAllCase {
            name: "import_alias_control_flow_nested_call_root_aggregate_match_catch_all_build",
            source_relative: "fixtures/codegen/pass/import_alias_control_flow_nested_call_root_aggregate_match_catch_all.ql",
            context: "import-alias control-flow nested call-root aggregate match catch-all build regression",
        },
    ]
}
