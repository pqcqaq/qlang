mod support;

use support::codegen::{PassCase, run_pass_case};
use support::workspace_root;

struct LinkedArtifactCase {
    name: &'static str,
    source_relative: &'static str,
    emit: &'static str,
    expected_relative: &'static str,
    header_surface: Option<&'static str>,
    expected_header_relative: Option<&'static str>,
}

#[test]
fn executable_and_dylib_codegen_cases_match() {
    let workspace_root = workspace_root();
    let mut failures = Vec::new();

    for case in linked_artifact_cases() {
        let pass_case = PassCase {
            name: case.name,
            source_relative: case.source_relative,
            emit: case.emit,
            expected_relative: case.expected_relative,
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: case.header_surface,
            expected_header_relative: case.expected_header_relative,
        };

        if let Err(message) = run_pass_case(&workspace_root, &pass_case) {
            failures.push(format!("{}:\n\n{}", case.name, message));
        }
    }

    assert!(
        failures.is_empty(),
        "executable/dylib codegen regressions:\n\n{}",
        failures.join("\n\n")
    );
}

fn linked_artifact_cases() -> &'static [LinkedArtifactCase] {
    &[
        LinkedArtifactCase {
            name: "minimal_build_executable",
            source_relative: "fixtures/codegen/pass/minimal_build.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            header_surface: None,
            expected_header_relative: None,
        },
        LinkedArtifactCase {
            name: "extern_c_export_dylib",
            source_relative: "fixtures/codegen/pass/extern_c_export.ql",
            emit: "dylib",
            expected_relative: "tests/codegen/pass/extern_c_export.dylib.txt",
            header_surface: None,
            expected_header_relative: None,
        },
        LinkedArtifactCase {
            name: "extern_c_export_dylib_with_header",
            source_relative: "fixtures/codegen/pass/extern_c_export.ql",
            emit: "dylib",
            expected_relative: "tests/codegen/pass/extern_c_export.dylib.txt",
            header_surface: Some("exports"),
            expected_header_relative: Some("tests/codegen/pass/extern_c_export.h"),
        },
        LinkedArtifactCase {
            name: "ffi_export_async_dylib",
            source_relative: "fixtures/codegen/pass/ffi_export_async.ql",
            emit: "dylib",
            expected_relative: "tests/codegen/pass/extern_c_export.dylib.txt",
            header_surface: None,
            expected_header_relative: None,
        },
        LinkedArtifactCase {
            name: "ffi_export_async_dylib_with_header",
            source_relative: "fixtures/codegen/pass/ffi_export_async.ql",
            emit: "dylib",
            expected_relative: "tests/codegen/pass/extern_c_export.dylib.txt",
            header_surface: Some("exports"),
            expected_header_relative: Some("tests/codegen/pass/ffi_export_async.h"),
        },
        LinkedArtifactCase {
            name: "ffi_export_async_for_await_array_dylib",
            source_relative: "fixtures/codegen/pass/ffi_export_async_for_await_array.ql",
            emit: "dylib",
            expected_relative: "tests/codegen/pass/extern_c_export.dylib.txt",
            header_surface: None,
            expected_header_relative: None,
        },
        LinkedArtifactCase {
            name: "ffi_export_async_for_await_tuple_dylib",
            source_relative: "fixtures/codegen/pass/ffi_export_async_for_await_tuple.ql",
            emit: "dylib",
            expected_relative: "tests/codegen/pass/extern_c_export.dylib.txt",
            header_surface: None,
            expected_header_relative: None,
        },
        LinkedArtifactCase {
            name: "ffi_export_async_task_tuple_for_await_dylib",
            source_relative: "fixtures/codegen/pass/ffi_export_async_task_tuple_for_await.ql",
            emit: "dylib",
            expected_relative: "tests/codegen/pass/extern_c_export.dylib.txt",
            header_surface: None,
            expected_header_relative: None,
        },
        LinkedArtifactCase {
            name: "ffi_export_async_task_array_for_await_dylib",
            source_relative: "fixtures/codegen/pass/ffi_export_async_task_array_for_await.ql",
            emit: "dylib",
            expected_relative: "tests/codegen/pass/extern_c_export.dylib.txt",
            header_surface: None,
            expected_header_relative: None,
        },
        LinkedArtifactCase {
            name: "ffi_export_async_aggregate_await_families_dylib",
            source_relative: "fixtures/codegen/pass/ffi_export_async_aggregate_await_families.ql",
            emit: "dylib",
            expected_relative: "tests/codegen/pass/extern_c_export.dylib.txt",
            header_surface: None,
            expected_header_relative: None,
        },
        LinkedArtifactCase {
            name: "ffi_export_async_aggregate_param_result_families_dylib",
            source_relative: "fixtures/codegen/pass/ffi_export_async_aggregate_param_result_families.ql",
            emit: "dylib",
            expected_relative: "tests/codegen/pass/extern_c_export.dylib.txt",
            header_surface: None,
            expected_header_relative: None,
        },
        LinkedArtifactCase {
            name: "ffi_export_async_match_families_dylib",
            source_relative: "fixtures/codegen/pass/ffi_export_async_match_families.ql",
            emit: "dylib",
            expected_relative: "tests/codegen/pass/extern_c_export.dylib.txt",
            header_surface: None,
            expected_header_relative: None,
        },
        LinkedArtifactCase {
            name: "ffi_export_async_task_handle_payload_families_dylib",
            source_relative: "fixtures/codegen/pass/ffi_export_async_task_handle_payload_families.ql",
            emit: "dylib",
            expected_relative: "tests/codegen/pass/extern_c_export.dylib.txt",
            header_surface: None,
            expected_header_relative: None,
        },
        LinkedArtifactCase {
            name: "ffi_export_async_task_handle_flow_families_dylib",
            source_relative: "fixtures/codegen/pass/ffi_export_async_task_handle_flow_families.ql",
            emit: "dylib",
            expected_relative: "tests/codegen/pass/extern_c_export.dylib.txt",
            header_surface: None,
            expected_header_relative: None,
        },
        LinkedArtifactCase {
            name: "ffi_export_async_dynamic_task_handle_paths_dylib",
            source_relative: "fixtures/codegen/pass/ffi_export_async_dynamic_task_handle_paths.ql",
            emit: "dylib",
            expected_relative: "tests/codegen/pass/extern_c_export.dylib.txt",
            header_surface: None,
            expected_header_relative: None,
        },
        LinkedArtifactCase {
            name: "ffi_export_async_aliased_projected_root_repackage_families_dylib",
            source_relative: "fixtures/codegen/pass/ffi_export_async_aliased_projected_root_repackage_families.ql",
            emit: "dylib",
            expected_relative: "tests/codegen/pass/extern_c_export.dylib.txt",
            header_surface: None,
            expected_header_relative: None,
        },
        LinkedArtifactCase {
            name: "ffi_export_async_aliased_projected_root_spawn_families_dylib",
            source_relative: "fixtures/codegen/pass/ffi_export_async_aliased_projected_root_spawn_families.ql",
            emit: "dylib",
            expected_relative: "tests/codegen/pass/extern_c_export.dylib.txt",
            header_surface: None,
            expected_header_relative: None,
        },
        LinkedArtifactCase {
            name: "ffi_export_async_guard_refined_dynamic_path_families_dylib",
            source_relative: "fixtures/codegen/pass/ffi_export_async_guard_refined_dynamic_path_families.ql",
            emit: "dylib",
            expected_relative: "tests/codegen/pass/extern_c_export.dylib.txt",
            header_surface: None,
            expected_header_relative: None,
        },
        LinkedArtifactCase {
            name: "ffi_export_async_projected_reinit_families_dylib",
            source_relative: "fixtures/codegen/pass/ffi_export_async_projected_reinit_families.ql",
            emit: "dylib",
            expected_relative: "tests/codegen/pass/extern_c_export.dylib.txt",
            header_surface: None,
            expected_header_relative: None,
        },
        LinkedArtifactCase {
            name: "ffi_export_async_task_handle_consume_families_dylib",
            source_relative: "fixtures/codegen/pass/ffi_export_async_task_handle_consume_families.ql",
            emit: "dylib",
            expected_relative: "tests/codegen/pass/extern_c_export.dylib.txt",
            header_surface: None,
            expected_header_relative: None,
        },
        LinkedArtifactCase {
            name: "ffi_export_async_inline_without_parens_for_await_dylib",
            source_relative: "fixtures/codegen/pass/ffi_export_async_inline_without_parens_for_await.ql",
            emit: "dylib",
            expected_relative: "tests/codegen/pass/extern_c_export.dylib.txt",
            header_surface: None,
            expected_header_relative: None,
        },
        LinkedArtifactCase {
            name: "ffi_export_async_inline_for_await_dylib",
            source_relative: "fixtures/codegen/pass/ffi_export_async_inline_for_await.ql",
            emit: "dylib",
            expected_relative: "tests/codegen/pass/extern_c_export.dylib.txt",
            header_surface: None,
            expected_header_relative: None,
        },
        LinkedArtifactCase {
            name: "ffi_export_async_import_alias_awaited_for_await_dylib",
            source_relative: "fixtures/codegen/pass/ffi_export_async_import_alias_awaited_for_await.ql",
            emit: "dylib",
            expected_relative: "tests/codegen/pass/extern_c_export.dylib.txt",
            header_surface: None,
            expected_header_relative: None,
        },
        LinkedArtifactCase {
            name: "ffi_export_async_import_alias_for_await_dylib",
            source_relative: "fixtures/codegen/pass/ffi_export_async_import_alias_for_await.ql",
            emit: "dylib",
            expected_relative: "tests/codegen/pass/extern_c_export.dylib.txt",
            header_surface: None,
            expected_header_relative: None,
        },
        LinkedArtifactCase {
            name: "ffi_export_async_nested_call_root_for_await_dylib",
            source_relative: "fixtures/codegen/pass/ffi_export_async_nested_call_root_for_await.ql",
            emit: "dylib",
            expected_relative: "tests/codegen/pass/extern_c_export.dylib.txt",
            header_surface: None,
            expected_header_relative: None,
        },
        LinkedArtifactCase {
            name: "ffi_export_async_awaited_projected_for_await_dylib",
            source_relative: "fixtures/codegen/pass/ffi_export_async_awaited_projected_for_await.ql",
            emit: "dylib",
            expected_relative: "tests/codegen/pass/extern_c_export.dylib.txt",
            header_surface: None,
            expected_header_relative: None,
        },
        LinkedArtifactCase {
            name: "ffi_export_async_call_root_for_await_dylib",
            source_relative: "fixtures/codegen/pass/ffi_export_async_call_root_for_await.ql",
            emit: "dylib",
            expected_relative: "tests/codegen/pass/extern_c_export.dylib.txt",
            header_surface: None,
            expected_header_relative: None,
        },
        LinkedArtifactCase {
            name: "ffi_export_async_projected_for_await_dylib",
            source_relative: "fixtures/codegen/pass/ffi_export_async_projected_for_await.ql",
            emit: "dylib",
            expected_relative: "tests/codegen/pass/extern_c_export.dylib.txt",
            header_surface: None,
            expected_header_relative: None,
        },
    ]
}
