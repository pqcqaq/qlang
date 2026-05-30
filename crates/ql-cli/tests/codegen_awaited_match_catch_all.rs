mod support;

use support::codegen::{PassCase, run_pass_case};
use support::workspace_root;

struct AwaitedCatchAllCase {
    name: &'static str,
    source_relative: &'static str,
    context: &'static str,
}

#[test]
fn awaited_match_catch_all_codegen_cases_match() {
    let workspace_root = workspace_root();
    let mut failures = Vec::new();

    for case in awaited_match_catch_all_cases() {
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
        "awaited match catch-all codegen regressions:\n\n{}",
        failures.join("\n\n")
    );
}

fn awaited_match_catch_all_cases() -> &'static [AwaitedCatchAllCase] {
    &[
        AwaitedCatchAllCase {
            name: "awaited_call_root_aggregate_match_catch_all_build",
            source_relative: "fixtures/codegen/pass/awaited_call_root_aggregate_match_catch_all.ql",
            context: "awaited call-root aggregate match catch-all build regression",
        },
        AwaitedCatchAllCase {
            name: "import_alias_awaited_call_root_aggregate_match_catch_all_build",
            source_relative: "fixtures/codegen/pass/import_alias_awaited_call_root_aggregate_match_catch_all.ql",
            context: "import-alias awaited call-root aggregate match catch-all build regression",
        },
        AwaitedCatchAllCase {
            name: "control_flow_awaited_call_root_aggregate_match_catch_all_build",
            source_relative: "fixtures/codegen/pass/control_flow_awaited_call_root_aggregate_match_catch_all.ql",
            context: "control-flow awaited call-root aggregate match catch-all build regression",
        },
        AwaitedCatchAllCase {
            name: "import_alias_control_flow_awaited_call_root_aggregate_match_catch_all_build",
            source_relative: "fixtures/codegen/pass/import_alias_control_flow_awaited_call_root_aggregate_match_catch_all.ql",
            context: "import-alias control-flow awaited call-root aggregate match catch-all build regression",
        },
        AwaitedCatchAllCase {
            name: "awaited_projected_aggregate_match_catch_all_build",
            source_relative: "fixtures/codegen/pass/awaited_projected_aggregate_match_catch_all.ql",
            context: "awaited projected aggregate match catch-all build regression",
        },
        AwaitedCatchAllCase {
            name: "import_alias_awaited_projected_aggregate_match_catch_all_build",
            source_relative: "fixtures/codegen/pass/import_alias_awaited_projected_aggregate_match_catch_all.ql",
            context: "import-alias awaited projected aggregate match catch-all build regression",
        },
        AwaitedCatchAllCase {
            name: "control_flow_awaited_projected_aggregate_match_catch_all_build",
            source_relative: "fixtures/codegen/pass/control_flow_awaited_projected_aggregate_match_catch_all.ql",
            context: "control-flow awaited projected aggregate match catch-all build regression",
        },
        AwaitedCatchAllCase {
            name: "import_alias_control_flow_awaited_projected_aggregate_match_catch_all_build",
            source_relative: "fixtures/codegen/pass/import_alias_control_flow_awaited_projected_aggregate_match_catch_all.ql",
            context: "import-alias control-flow awaited projected aggregate match catch-all build regression",
        },
        AwaitedCatchAllCase {
            name: "awaited_nested_projected_aggregate_match_catch_all_build",
            source_relative: "fixtures/codegen/pass/awaited_nested_projected_aggregate_match_catch_all.ql",
            context: "awaited nested projected aggregate match catch-all build regression",
        },
        AwaitedCatchAllCase {
            name: "import_alias_awaited_nested_projected_aggregate_match_catch_all_build",
            source_relative: "fixtures/codegen/pass/import_alias_awaited_nested_projected_aggregate_match_catch_all.ql",
            context: "import-alias awaited nested projected aggregate match catch-all build regression",
        },
        AwaitedCatchAllCase {
            name: "control_flow_awaited_nested_projected_aggregate_match_catch_all_build",
            source_relative: "fixtures/codegen/pass/control_flow_awaited_nested_projected_aggregate_match_catch_all.ql",
            context: "control-flow awaited nested projected aggregate match catch-all build regression",
        },
        AwaitedCatchAllCase {
            name: "import_alias_control_flow_awaited_nested_projected_aggregate_match_catch_all_build",
            source_relative: "fixtures/codegen/pass/import_alias_control_flow_awaited_nested_projected_aggregate_match_catch_all.ql",
            context: "import-alias control-flow awaited nested projected aggregate match catch-all build regression",
        },
    ]
}
