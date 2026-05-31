mod support;

use support::codegen::{PassCase, run_pass_case};
use support::workspace_root;

struct CleanupQuestionMarkCase {
    name: &'static str,
    source_relative: &'static str,
    context: &'static str,
}

#[test]
fn cleanup_question_mark_codegen_cases_match() {
    let workspace_root = workspace_root();
    let mut failures = Vec::new();

    for case in cleanup_question_mark_cases() {
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
        "cleanup question-mark codegen regressions:\n\n{}",
        failures.join("\n\n")
    );
}

fn cleanup_question_mark_cases() -> &'static [CleanupQuestionMarkCase] {
    &[
        CleanupQuestionMarkCase {
            name: "match_question_mark_build",
            source_relative: "fixtures/codegen/pass/match_question_mark.ql",
            context: "match question-mark build regression",
        },
        CleanupQuestionMarkCase {
            name: "cleanup_question_mark_build",
            source_relative: "fixtures/codegen/pass/cleanup_question_mark.ql",
            context: "cleanup question-mark build regression",
        },
        CleanupQuestionMarkCase {
            name: "cleanup_internal_question_mark_build",
            source_relative: "fixtures/codegen/pass/cleanup_internal_question_mark.ql",
            context: "cleanup internal question-mark build regression",
        },
    ]
}
