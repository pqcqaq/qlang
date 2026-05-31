mod support;

use support::codegen::{PassCase, run_pass_case};
use support::workspace_root;

struct CallRootFixedShapeIrCase {
    name: &'static str,
    source_relative: &'static str,
    expected_relative: &'static str,
    context: &'static str,
}

#[test]
fn call_root_fixed_shape_ir_codegen_cases_match() {
    let workspace_root = workspace_root();
    let mut failures = Vec::new();

    for case in call_root_fixed_shape_ir_cases() {
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
        "call-root fixed-shape LLVM IR codegen regressions:\n\n{}",
        failures.join("\n\n")
    );
}

fn call_root_fixed_shape_ir_cases() -> &'static [CallRootFixedShapeIrCase] {
    &[
        CallRootFixedShapeIrCase {
            name: "for_call_root_fixed_shapes_llvm_ir",
            source_relative: "fixtures/codegen/pass/for_call_root_fixed_shapes.ql",
            expected_relative: "tests/codegen/pass/for_call_root_fixed_shapes.ll",
            context: "for call-root fixed-shape LLVM IR regression",
        },
        CallRootFixedShapeIrCase {
            name: "import_alias_call_root_fixed_shapes_llvm_ir",
            source_relative: "fixtures/codegen/pass/import_alias_call_root_fixed_shapes.ql",
            expected_relative: "tests/codegen/pass/import_alias_call_root_fixed_shapes.ll",
            context: "import alias call-root fixed-shape LLVM IR regression",
        },
        CallRootFixedShapeIrCase {
            name: "nested_call_root_fixed_shapes_llvm_ir",
            source_relative: "fixtures/codegen/pass/nested_call_root_fixed_shapes.ql",
            expected_relative: "tests/codegen/pass/nested_call_root_fixed_shapes.ll",
            context: "nested call-root fixed-shape LLVM IR regression",
        },
        CallRootFixedShapeIrCase {
            name: "import_alias_nested_call_root_fixed_shapes_llvm_ir",
            source_relative: "fixtures/codegen/pass/import_alias_nested_call_root_fixed_shapes.ql",
            expected_relative: "tests/codegen/pass/import_alias_nested_call_root_fixed_shapes.ll",
            context: "import alias nested call-root fixed-shape LLVM IR regression",
        },
    ]
}
