mod support;

use support::codegen::{PassCase, run_pass_case};
use support::workspace_root;

struct AsyncClosureIrCase {
    name: &'static str,
    source_relative: &'static str,
    expected_relative: &'static str,
    context: &'static str,
}

#[test]
fn async_closure_ir_codegen_cases_match() {
    let workspace_root = workspace_root();
    let mut failures = Vec::new();

    for case in async_closure_ir_cases() {
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
        "async/closure LLVM IR codegen regressions:\n\n{}",
        failures.join("\n\n")
    );
}

fn async_closure_ir_cases() -> &'static [AsyncClosureIrCase] {
    &[
        AsyncClosureIrCase {
            name: "async_function_value_build_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_function_value_build.ql",
            expected_relative: "tests/codegen/pass/async_function_value_build.ll",
            context: "async function-value LLVM IR regression",
        },
        AsyncClosureIrCase {
            name: "async_callable_const_static_build_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_callable_const_static_build.ql",
            expected_relative: "tests/codegen/pass/async_callable_const_static_build.ll",
            context: "async callable const/static LLVM IR regression",
        },
        AsyncClosureIrCase {
            name: "non_capturing_closure_build_llvm_ir",
            source_relative: "fixtures/codegen/pass/non_capturing_closure_build.ql",
            expected_relative: "tests/codegen/pass/non_capturing_closure_build.ll",
            context: "non-capturing closure LLVM IR regression",
        },
        AsyncClosureIrCase {
            name: "non_capturing_param_closure_build_llvm_ir",
            source_relative: "fixtures/codegen/pass/non_capturing_param_closure_build.ql",
            expected_relative: "tests/codegen/pass/non_capturing_param_closure_build.ll",
            context: "non-capturing param closure LLVM IR regression",
        },
        AsyncClosureIrCase {
            name: "typed_non_capturing_param_closure_build_llvm_ir",
            source_relative: "fixtures/codegen/pass/typed_non_capturing_param_closure_build.ql",
            expected_relative: "tests/codegen/pass/typed_non_capturing_param_closure_build.ll",
            context: "typed non-capturing param closure LLVM IR regression",
        },
        AsyncClosureIrCase {
            name: "annotated_local_closure_build_llvm_ir",
            source_relative: "fixtures/codegen/pass/annotated_local_closure_build.ql",
            expected_relative: "tests/codegen/pass/annotated_local_closure_build.ll",
            context: "annotated local closure LLVM IR regression",
        },
        AsyncClosureIrCase {
            name: "closure_backed_callable_const_static_build_llvm_ir",
            source_relative: "fixtures/codegen/pass/closure_backed_callable_const_static_build.ql",
            expected_relative: "tests/codegen/pass/closure_backed_callable_const_static_build.ll",
            context: "closure-backed callable const/static LLVM IR regression",
        },
    ]
}
