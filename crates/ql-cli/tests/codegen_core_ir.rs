mod support;

use support::codegen::{PassCase, run_pass_case};
use support::workspace_root;

struct CoreIrCase {
    name: &'static str,
    source_relative: &'static str,
    expected_relative: &'static str,
    context: &'static str,
}

#[test]
fn core_ir_codegen_cases_match() {
    let workspace_root = workspace_root();
    let mut failures = Vec::new();

    for case in core_ir_cases() {
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
        "core LLVM IR codegen regressions:\n\n{}",
        failures.join("\n\n")
    );
}

fn core_ir_cases() -> &'static [CoreIrCase] {
    &[
        CoreIrCase {
            name: "minimal_build_llvm_ir",
            source_relative: "fixtures/codegen/pass/minimal_build.ql",
            expected_relative: "tests/codegen/pass/minimal_build.ll",
            context: "minimal build LLVM IR regression",
        },
        CoreIrCase {
            name: "extern_c_build_llvm_ir",
            source_relative: "fixtures/codegen/pass/extern_c_build.ql",
            expected_relative: "tests/codegen/pass/extern_c_build.ll",
            context: "extern C build LLVM IR regression",
        },
        CoreIrCase {
            name: "extern_c_export_llvm_ir",
            source_relative: "fixtures/codegen/pass/extern_c_export.ql",
            expected_relative: "tests/codegen/pass/extern_c_export.ll",
            context: "extern C export LLVM IR regression",
        },
        CoreIrCase {
            name: "projection_reads_llvm_ir",
            source_relative: "fixtures/codegen/pass/projection_reads.ql",
            expected_relative: "tests/codegen/pass/projection_reads.ll",
            context: "projection read LLVM IR regression",
        },
        CoreIrCase {
            name: "nested_projection_reads_llvm_ir",
            source_relative: "fixtures/codegen/pass/nested_projection_reads.ql",
            expected_relative: "tests/codegen/pass/nested_projection_reads.ll",
            context: "nested projection read LLVM IR regression",
        },
        CoreIrCase {
            name: "empty_array_expected_llvm_ir",
            source_relative: "fixtures/codegen/pass/empty_array_expected.ql",
            expected_relative: "tests/codegen/pass/empty_array_expected.ll",
            context: "empty array expected LLVM IR regression",
        },
        CoreIrCase {
            name: "for_array_llvm_ir",
            source_relative: "fixtures/codegen/pass/for_array.ql",
            expected_relative: "tests/codegen/pass/for_array.ll",
            context: "for-array LLVM IR regression",
        },
    ]
}
