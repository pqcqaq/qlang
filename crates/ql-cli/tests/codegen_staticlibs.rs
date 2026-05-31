mod support;

use support::codegen::{PassCase, current_archiver_style, run_pass_case};
use support::workspace_root;

struct StaticlibCase {
    name: &'static str,
    source_relative: &'static str,
    expected_relative: &'static str,
    header_surface: Option<&'static str>,
    expected_header_relative: Option<&'static str>,
}

#[test]
fn staticlib_codegen_cases_match() {
    let workspace_root = workspace_root();
    let mut failures = Vec::new();

    for case in staticlib_cases() {
        let pass_case = PassCase {
            name: case.name,
            source_relative: case.source_relative,
            emit: "staticlib",
            expected_relative: case.expected_relative,
            mock_compiler: true,
            mock_archiver: true,
            archiver_style: Some(current_archiver_style()),
            header_surface: case.header_surface,
            expected_header_relative: case.expected_header_relative,
        };

        if let Err(message) = run_pass_case(&workspace_root, &pass_case) {
            failures.push(format!("{}:\n\n{}", case.name, message));
        }
    }

    assert!(
        failures.is_empty(),
        "staticlib codegen regressions:\n\n{}",
        failures.join("\n\n")
    );
}

fn staticlib_cases() -> &'static [StaticlibCase] {
    &[
        StaticlibCase {
            name: "minimal_library_staticlib",
            source_relative: "fixtures/codegen/pass/minimal_library.ql",
            expected_relative: "tests/codegen/pass/minimal_library.staticlib.txt",
            header_surface: None,
            expected_header_relative: None,
        },
        StaticlibCase {
            name: "dynamic_array_assignment_staticlib",
            source_relative: "fixtures/codegen/pass/dynamic_array_assignment.ql",
            expected_relative: "tests/codegen/pass/minimal_library.staticlib.txt",
            header_surface: None,
            expected_header_relative: None,
        },
        StaticlibCase {
            name: "dynamic_nested_array_assignment_staticlib",
            source_relative: "fixtures/codegen/pass/dynamic_nested_array_assignment.ql",
            expected_relative: "tests/codegen/pass/minimal_library.staticlib.txt",
            header_surface: None,
            expected_header_relative: None,
        },
        StaticlibCase {
            name: "async_dynamic_task_array_assignment_staticlib",
            source_relative: "fixtures/codegen/pass/async_dynamic_task_array_assignment.ql",
            expected_relative: "tests/codegen/pass/minimal_library.staticlib.txt",
            header_surface: None,
            expected_header_relative: None,
        },
        StaticlibCase {
            name: "async_library_aggregate_param_result_families_staticlib",
            source_relative: "fixtures/codegen/pass/async_library_aggregate_param_result_families.ql",
            expected_relative: "tests/codegen/pass/minimal_library.staticlib.txt",
            header_surface: None,
            expected_header_relative: None,
        },
        StaticlibCase {
            name: "async_library_match_families_staticlib",
            source_relative: "fixtures/codegen/pass/async_library_match_families.ql",
            expected_relative: "tests/codegen/pass/minimal_library.staticlib.txt",
            header_surface: None,
            expected_header_relative: None,
        },
        StaticlibCase {
            name: "async_library_spawn_staticlib",
            source_relative: "fixtures/codegen/pass/async_library_spawn.ql",
            expected_relative: "tests/codegen/pass/minimal_library.staticlib.txt",
            header_surface: None,
            expected_header_relative: None,
        },
        StaticlibCase {
            name: "async_library_for_await_array_staticlib",
            source_relative: "fixtures/codegen/pass/async_library_for_await_array.ql",
            expected_relative: "tests/codegen/pass/minimal_library.staticlib.txt",
            header_surface: None,
            expected_header_relative: None,
        },
        StaticlibCase {
            name: "async_library_for_await_tuple_staticlib",
            source_relative: "fixtures/codegen/pass/async_library_for_await_tuple.ql",
            expected_relative: "tests/codegen/pass/minimal_library.staticlib.txt",
            header_surface: None,
            expected_header_relative: None,
        },
        StaticlibCase {
            name: "async_library_task_array_for_await_staticlib",
            source_relative: "fixtures/codegen/pass/async_library_task_array_for_await.ql",
            expected_relative: "tests/codegen/pass/minimal_library.staticlib.txt",
            header_surface: None,
            expected_header_relative: None,
        },
        StaticlibCase {
            name: "async_library_task_tuple_for_await_staticlib",
            source_relative: "fixtures/codegen/pass/async_library_task_tuple_for_await.ql",
            expected_relative: "tests/codegen/pass/minimal_library.staticlib.txt",
            header_surface: None,
            expected_header_relative: None,
        },
        StaticlibCase {
            name: "async_library_inline_without_parens_for_await_staticlib",
            source_relative: "fixtures/codegen/pass/async_library_inline_without_parens_for_await.ql",
            expected_relative: "tests/codegen/pass/minimal_library.staticlib.txt",
            header_surface: None,
            expected_header_relative: None,
        },
        StaticlibCase {
            name: "async_library_inline_for_await_staticlib",
            source_relative: "fixtures/codegen/pass/async_library_inline_for_await.ql",
            expected_relative: "tests/codegen/pass/minimal_library.staticlib.txt",
            header_surface: None,
            expected_header_relative: None,
        },
        StaticlibCase {
            name: "async_library_import_alias_awaited_for_await_staticlib",
            source_relative: "fixtures/codegen/pass/async_library_import_alias_awaited_for_await.ql",
            expected_relative: "tests/codegen/pass/minimal_library.staticlib.txt",
            header_surface: None,
            expected_header_relative: None,
        },
        StaticlibCase {
            name: "async_library_import_alias_for_await_staticlib",
            source_relative: "fixtures/codegen/pass/async_library_import_alias_for_await.ql",
            expected_relative: "tests/codegen/pass/minimal_library.staticlib.txt",
            header_surface: None,
            expected_header_relative: None,
        },
        StaticlibCase {
            name: "async_library_nested_call_root_for_await_staticlib",
            source_relative: "fixtures/codegen/pass/async_library_nested_call_root_for_await.ql",
            expected_relative: "tests/codegen/pass/minimal_library.staticlib.txt",
            header_surface: None,
            expected_header_relative: None,
        },
        StaticlibCase {
            name: "async_library_awaited_projected_for_await_staticlib",
            source_relative: "fixtures/codegen/pass/async_library_awaited_projected_for_await.ql",
            expected_relative: "tests/codegen/pass/minimal_library.staticlib.txt",
            header_surface: None,
            expected_header_relative: None,
        },
        StaticlibCase {
            name: "async_library_call_root_for_await_staticlib",
            source_relative: "fixtures/codegen/pass/async_library_call_root_for_await.ql",
            expected_relative: "tests/codegen/pass/minimal_library.staticlib.txt",
            header_surface: None,
            expected_header_relative: None,
        },
        StaticlibCase {
            name: "async_library_projected_for_await_staticlib",
            source_relative: "fixtures/codegen/pass/async_library_projected_for_await.ql",
            expected_relative: "tests/codegen/pass/minimal_library.staticlib.txt",
            header_surface: None,
            expected_header_relative: None,
        },
        StaticlibCase {
            name: "async_library_aggregate_await_families_staticlib",
            source_relative: "fixtures/codegen/pass/async_library_aggregate_await_families.ql",
            expected_relative: "tests/codegen/pass/minimal_library.staticlib.txt",
            header_surface: None,
            expected_header_relative: None,
        },
        StaticlibCase {
            name: "async_library_task_handle_payload_families_staticlib",
            source_relative: "fixtures/codegen/pass/async_library_task_handle_payload_families.ql",
            expected_relative: "tests/codegen/pass/minimal_library.staticlib.txt",
            header_surface: None,
            expected_header_relative: None,
        },
        StaticlibCase {
            name: "async_library_task_handle_flow_families_staticlib",
            source_relative: "fixtures/codegen/pass/async_library_task_handle_flow_families.ql",
            expected_relative: "tests/codegen/pass/minimal_library.staticlib.txt",
            header_surface: None,
            expected_header_relative: None,
        },
        StaticlibCase {
            name: "async_library_dynamic_task_handle_paths_staticlib",
            source_relative: "fixtures/codegen/pass/async_library_dynamic_task_handle_paths.ql",
            expected_relative: "tests/codegen/pass/minimal_library.staticlib.txt",
            header_surface: None,
            expected_header_relative: None,
        },
        StaticlibCase {
            name: "async_library_aliased_projected_root_repackage_families_staticlib",
            source_relative: "fixtures/codegen/pass/async_library_aliased_projected_root_repackage_families.ql",
            expected_relative: "tests/codegen/pass/minimal_library.staticlib.txt",
            header_surface: None,
            expected_header_relative: None,
        },
        StaticlibCase {
            name: "async_library_aliased_projected_root_spawn_families_staticlib",
            source_relative: "fixtures/codegen/pass/async_library_aliased_projected_root_spawn_families.ql",
            expected_relative: "tests/codegen/pass/minimal_library.staticlib.txt",
            header_surface: None,
            expected_header_relative: None,
        },
        StaticlibCase {
            name: "async_library_guard_refined_dynamic_path_families_staticlib",
            source_relative: "fixtures/codegen/pass/async_library_guard_refined_dynamic_path_families.ql",
            expected_relative: "tests/codegen/pass/minimal_library.staticlib.txt",
            header_surface: None,
            expected_header_relative: None,
        },
        StaticlibCase {
            name: "async_library_projected_reinit_families_staticlib",
            source_relative: "fixtures/codegen/pass/async_library_projected_reinit_families.ql",
            expected_relative: "tests/codegen/pass/minimal_library.staticlib.txt",
            header_surface: None,
            expected_header_relative: None,
        },
        StaticlibCase {
            name: "async_library_task_handle_consume_families_staticlib",
            source_relative: "fixtures/codegen/pass/async_library_task_handle_consume_families.ql",
            expected_relative: "tests/codegen/pass/minimal_library.staticlib.txt",
            header_surface: None,
            expected_header_relative: None,
        },
        StaticlibCase {
            name: "async_library_spawn_zero_sized_aggregate_result_staticlib",
            source_relative: "fixtures/codegen/pass/async_library_spawn_zero_sized_aggregate_result.ql",
            expected_relative: "tests/codegen/pass/minimal_library.staticlib.txt",
            header_surface: None,
            expected_header_relative: None,
        },
        StaticlibCase {
            name: "ffi_export_async_staticlib_with_header",
            source_relative: "fixtures/codegen/pass/ffi_export_async.ql",
            expected_relative: "tests/codegen/pass/minimal_library.staticlib.txt",
            header_surface: Some("exports"),
            expected_header_relative: Some("tests/codegen/pass/ffi_export_async.h"),
        },
        StaticlibCase {
            name: "extern_c_library_staticlib",
            source_relative: "fixtures/codegen/pass/extern_c_library.ql",
            expected_relative: "tests/codegen/pass/extern_c_library.staticlib.txt",
            header_surface: None,
            expected_header_relative: None,
        },
        StaticlibCase {
            name: "extern_c_library_staticlib_with_import_header",
            source_relative: "fixtures/codegen/pass/extern_c_library.ql",
            expected_relative: "tests/codegen/pass/extern_c_library.staticlib.txt",
            header_surface: Some("imports"),
            expected_header_relative: Some("tests/codegen/pass/extern_c_library.imports.h"),
        },
        StaticlibCase {
            name: "extern_c_import_top_level_staticlib_with_both_header",
            source_relative: "tests/ffi/pass/extern_c_import_top_level.ql",
            expected_relative: "tests/codegen/pass/extern_c_import_top_level.staticlib.txt",
            header_surface: Some("both"),
            expected_header_relative: Some("tests/codegen/pass/extern_c_import_top_level.ffi.h"),
        },
        StaticlibCase {
            name: "extern_c_top_level_library_staticlib",
            source_relative: "fixtures/codegen/pass/extern_c_top_level_library.ql",
            expected_relative: "tests/codegen/pass/extern_c_top_level_library.staticlib.txt",
            header_surface: None,
            expected_header_relative: None,
        },
    ]
}
