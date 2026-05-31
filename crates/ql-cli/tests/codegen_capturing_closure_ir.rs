mod support;

use support::codegen::{PassCase, run_pass_case};
use support::workspace_root;

#[test]
fn capturing_closure_ir_codegen_cases_match() {
    let workspace_root = workspace_root();
    let mut failures = Vec::new();

    for stem in capturing_closure_ir_stems() {
        let source_relative = format!("fixtures/codegen/pass/{stem}.ql");
        let expected_relative = format!("tests/codegen/pass/{stem}.ll");
        let case_name = format!("{stem}_llvm_ir");
        let pass_case = PassCase {
            name: &case_name,
            source_relative: &source_relative,
            emit: "llvm-ir",
            expected_relative: &expected_relative,
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        };

        if let Err(message) = run_pass_case(&workspace_root, &pass_case) {
            failures.push(format!("{} LLVM IR regression:\n\n{}", stem, message));
        }
    }

    assert!(
        failures.is_empty(),
        "capturing closure LLVM IR codegen regressions:\n\n{}",
        failures.join("\n\n")
    );
}

fn capturing_closure_ir_stems() -> &'static [&'static str] {
    &[
        "capturing_closure_reassigned_alias_call",
        "cleanup_reassigned_capturing_closure_value",
    ]
}
