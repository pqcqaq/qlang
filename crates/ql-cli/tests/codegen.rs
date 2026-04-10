mod support;

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use support::{
    TempDir, expect_empty_stderr, expect_empty_stdout, expect_exit_code, expect_file_exists,
    expect_snapshot_matches, expect_success, normalize_trimmed, ql_command, read_normalized_file,
    read_normalized_trimmed_file, run_command_capture, workspace_root,
};

#[test]
fn codegen_snapshots_match() {
    let workspace_root = workspace_root();

    let mut pass_cases = vec![
        PassCase {
            name: "minimal_build_llvm_ir",
            source_relative: "fixtures/codegen/pass/minimal_build.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/minimal_build.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "extern_c_build_llvm_ir",
            source_relative: "fixtures/codegen/pass/extern_c_build.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/extern_c_build.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "extern_c_export_llvm_ir",
            source_relative: "fixtures/codegen/pass/extern_c_export.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/extern_c_export.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "projection_reads_llvm_ir",
            source_relative: "fixtures/codegen/pass/projection_reads.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/projection_reads.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "nested_projection_reads_llvm_ir",
            source_relative: "fixtures/codegen/pass/nested_projection_reads.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/nested_projection_reads.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "empty_array_expected_llvm_ir",
            source_relative: "fixtures/codegen/pass/empty_array_expected.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/empty_array_expected.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "for_array_llvm_ir",
            source_relative: "fixtures/codegen/pass/for_array.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/for_array.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_function_value_build_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_function_value_build.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_function_value_build.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_callable_const_static_build_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_callable_const_static_build.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_callable_const_static_build.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "non_capturing_closure_build_llvm_ir",
            source_relative: "fixtures/codegen/pass/non_capturing_closure_build.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/non_capturing_closure_build.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "non_capturing_param_closure_build_llvm_ir",
            source_relative: "fixtures/codegen/pass/non_capturing_param_closure_build.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/non_capturing_param_closure_build.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "typed_non_capturing_param_closure_build_llvm_ir",
            source_relative: "fixtures/codegen/pass/typed_non_capturing_param_closure_build.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/typed_non_capturing_param_closure_build.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "annotated_local_closure_build_llvm_ir",
            source_relative: "fixtures/codegen/pass/annotated_local_closure_build.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/annotated_local_closure_build.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "closure_backed_callable_const_static_build_llvm_ir",
            source_relative: "fixtures/codegen/pass/closure_backed_callable_const_static_build.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/closure_backed_callable_const_static_build.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "bool_match_llvm_ir",
            source_relative: "fixtures/codegen/pass/bool_match.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/bool_match.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "integer_match_llvm_ir",
            source_relative: "fixtures/codegen/pass/integer_match.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/integer_match.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "integer_dynamic_guard_match_llvm_ir",
            source_relative: "fixtures/codegen/pass/integer_dynamic_guard_match.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/integer_dynamic_guard_match.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "integer_comparison_guard_match_llvm_ir",
            source_relative: "fixtures/codegen/pass/integer_comparison_guard_match.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/integer_comparison_guard_match.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "projected_integer_comparison_guard_match_llvm_ir",
            source_relative: "fixtures/codegen/pass/projected_integer_comparison_guard_match.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/projected_integer_comparison_guard_match.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "const_projected_integer_comparison_guard_match_llvm_ir",
            source_relative: "fixtures/codegen/pass/const_projected_integer_comparison_guard_match.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/const_projected_integer_comparison_guard_match.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "integer_dynamic_guard_catch_all_match_llvm_ir",
            source_relative: "fixtures/codegen/pass/integer_dynamic_guard_catch_all_match.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/integer_dynamic_guard_catch_all_match.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "integer_match_binding_llvm_ir",
            source_relative: "fixtures/codegen/pass/integer_match_binding.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/integer_match_binding.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "literal_guard_match_llvm_ir",
            source_relative: "fixtures/codegen/pass/literal_guard_match.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/literal_guard_match.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "const_guard_match_llvm_ir",
            source_relative: "fixtures/codegen/pass/const_guard_match.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/const_guard_match.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "bool_dynamic_guard_match_llvm_ir",
            source_relative: "fixtures/codegen/pass/bool_dynamic_guard_match.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/bool_dynamic_guard_match.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "negated_bool_guard_match_llvm_ir",
            source_relative: "fixtures/codegen/pass/negated_bool_guard_match.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/negated_bool_guard_match.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "bool_short_circuit_expr_llvm_ir",
            source_relative: "fixtures/codegen/pass/bool_short_circuit_expr.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/bool_short_circuit_expr.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "alias_const_guard_match_llvm_ir",
            source_relative: "fixtures/codegen/pass/alias_const_guard_match.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/alias_const_guard_match.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "match_guard_direct_calls_llvm_ir",
            source_relative: "fixtures/codegen/pass/match_guard_direct_calls.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/match_guard_direct_calls.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "match_guard_call_projection_roots_llvm_ir",
            source_relative: "fixtures/codegen/pass/match_guard_call_projection_roots.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/match_guard_call_projection_roots.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "match_guard_aggregate_call_args_llvm_ir",
            source_relative: "fixtures/codegen/pass/match_guard_aggregate_call_args.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/match_guard_aggregate_call_args.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "match_guard_inline_aggregate_call_args_llvm_ir",
            source_relative: "fixtures/codegen/pass/match_guard_inline_aggregate_call_args.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/match_guard_inline_aggregate_call_args.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "match_guard_inline_projection_roots_llvm_ir",
            source_relative: "fixtures/codegen/pass/match_guard_inline_projection_roots.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/match_guard_inline_projection_roots.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "match_guard_item_backed_inline_combos_llvm_ir",
            source_relative: "fixtures/codegen/pass/match_guard_item_backed_inline_combos.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/match_guard_item_backed_inline_combos.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "match_guard_call_backed_combos_llvm_ir",
            source_relative: "fixtures/codegen/pass/match_guard_call_backed_combos.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/match_guard_call_backed_combos.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "match_guard_call_root_nested_runtime_projection_llvm_ir",
            source_relative: "fixtures/codegen/pass/match_guard_call_root_nested_runtime_projection.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/match_guard_call_root_nested_runtime_projection.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "match_guard_nested_call_root_inline_combos_llvm_ir",
            source_relative: "fixtures/codegen/pass/match_guard_nested_call_root_inline_combos.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/match_guard_nested_call_root_inline_combos.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "match_guard_item_backed_nested_call_root_combos_llvm_ir",
            source_relative: "fixtures/codegen/pass/match_guard_item_backed_nested_call_root_combos.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/match_guard_item_backed_nested_call_root_combos.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "match_guard_call_backed_nested_call_root_combos_llvm_ir",
            source_relative: "fixtures/codegen/pass/match_guard_call_backed_nested_call_root_combos.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/match_guard_call_backed_nested_call_root_combos.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "match_guard_alias_backed_nested_call_root_combos_llvm_ir",
            source_relative: "fixtures/codegen/pass/match_guard_alias_backed_nested_call_root_combos.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/match_guard_alias_backed_nested_call_root_combos.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "match_guard_binding_backed_nested_call_root_combos_llvm_ir",
            source_relative: "fixtures/codegen/pass/match_guard_binding_backed_nested_call_root_combos.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/match_guard_binding_backed_nested_call_root_combos.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "match_guard_projection_backed_nested_call_root_combos_llvm_ir",
            source_relative: "fixtures/codegen/pass/match_guard_projection_backed_nested_call_root_combos.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/match_guard_projection_backed_nested_call_root_combos.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "for_call_root_fixed_shapes_llvm_ir",
            source_relative: "fixtures/codegen/pass/for_call_root_fixed_shapes.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/for_call_root_fixed_shapes.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "import_alias_call_root_fixed_shapes_llvm_ir",
            source_relative: "fixtures/codegen/pass/import_alias_call_root_fixed_shapes.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/import_alias_call_root_fixed_shapes.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "nested_call_root_fixed_shapes_llvm_ir",
            source_relative: "fixtures/codegen/pass/nested_call_root_fixed_shapes.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/nested_call_root_fixed_shapes.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "import_alias_nested_call_root_fixed_shapes_llvm_ir",
            source_relative: "fixtures/codegen/pass/import_alias_nested_call_root_fixed_shapes.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/import_alias_nested_call_root_fixed_shapes.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "minimal_build_object",
            source_relative: "fixtures/codegen/pass/minimal_build.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "bool_match_object",
            source_relative: "fixtures/codegen/pass/bool_match.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "integer_match_object",
            source_relative: "fixtures/codegen/pass/integer_match.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "integer_dynamic_guard_match_object",
            source_relative: "fixtures/codegen/pass/integer_dynamic_guard_match.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "integer_comparison_guard_match_object",
            source_relative: "fixtures/codegen/pass/integer_comparison_guard_match.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "projected_integer_comparison_guard_match_object",
            source_relative: "fixtures/codegen/pass/projected_integer_comparison_guard_match.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "const_projected_integer_comparison_guard_match_object",
            source_relative: "fixtures/codegen/pass/const_projected_integer_comparison_guard_match.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "integer_dynamic_guard_catch_all_match_object",
            source_relative: "fixtures/codegen/pass/integer_dynamic_guard_catch_all_match.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "integer_match_binding_object",
            source_relative: "fixtures/codegen/pass/integer_match_binding.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "literal_guard_match_object",
            source_relative: "fixtures/codegen/pass/literal_guard_match.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "const_guard_match_object",
            source_relative: "fixtures/codegen/pass/const_guard_match.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "bool_dynamic_guard_match_object",
            source_relative: "fixtures/codegen/pass/bool_dynamic_guard_match.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "negated_bool_guard_match_object",
            source_relative: "fixtures/codegen/pass/negated_bool_guard_match.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "alias_const_guard_match_object",
            source_relative: "fixtures/codegen/pass/alias_const_guard_match.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "match_guard_direct_calls_object",
            source_relative: "fixtures/codegen/pass/match_guard_direct_calls.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "match_guard_call_projection_roots_object",
            source_relative: "fixtures/codegen/pass/match_guard_call_projection_roots.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "match_guard_aggregate_call_args_object",
            source_relative: "fixtures/codegen/pass/match_guard_aggregate_call_args.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "match_guard_inline_aggregate_call_args_object",
            source_relative: "fixtures/codegen/pass/match_guard_inline_aggregate_call_args.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "match_guard_inline_projection_roots_object",
            source_relative: "fixtures/codegen/pass/match_guard_inline_projection_roots.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "match_guard_item_backed_inline_combos_object",
            source_relative: "fixtures/codegen/pass/match_guard_item_backed_inline_combos.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "match_guard_call_backed_combos_object",
            source_relative: "fixtures/codegen/pass/match_guard_call_backed_combos.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "match_guard_call_root_nested_runtime_projection_object",
            source_relative: "fixtures/codegen/pass/match_guard_call_root_nested_runtime_projection.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "match_guard_nested_call_root_inline_combos_object",
            source_relative: "fixtures/codegen/pass/match_guard_nested_call_root_inline_combos.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "match_guard_item_backed_nested_call_root_combos_object",
            source_relative: "fixtures/codegen/pass/match_guard_item_backed_nested_call_root_combos.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "match_guard_call_backed_nested_call_root_combos_object",
            source_relative: "fixtures/codegen/pass/match_guard_call_backed_nested_call_root_combos.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "match_guard_alias_backed_nested_call_root_combos_object",
            source_relative: "fixtures/codegen/pass/match_guard_alias_backed_nested_call_root_combos.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "match_guard_binding_backed_nested_call_root_combos_object",
            source_relative: "fixtures/codegen/pass/match_guard_binding_backed_nested_call_root_combos.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "match_guard_projection_backed_nested_call_root_combos_object",
            source_relative: "fixtures/codegen/pass/match_guard_projection_backed_nested_call_root_combos.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "for_call_root_fixed_shapes_object",
            source_relative: "fixtures/codegen/pass/for_call_root_fixed_shapes.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "import_alias_call_root_fixed_shapes_object",
            source_relative: "fixtures/codegen/pass/import_alias_call_root_fixed_shapes.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "nested_call_root_fixed_shapes_object",
            source_relative: "fixtures/codegen/pass/nested_call_root_fixed_shapes.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "import_alias_nested_call_root_fixed_shapes_object",
            source_relative: "fixtures/codegen/pass/import_alias_nested_call_root_fixed_shapes.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "for_array_object",
            source_relative: "fixtures/codegen/pass/for_array.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_spawn_bound_task_handle_object",
            source_relative: "fixtures/codegen/pass/async_program_main_spawn_bound_task_handle.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_for_await_array_object",
            source_relative: "fixtures/codegen/pass/async_program_main_for_await_array.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_guard_refined_dynamic_task_handle_reinit_object",
            source_relative: "fixtures/codegen/pass/async_program_main_guard_refined_dynamic_task_handle_reinit.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_guard_refined_projected_dynamic_task_handle_reinit_object",
            source_relative: "fixtures/codegen/pass/async_program_main_guard_refined_projected_dynamic_task_handle_reinit.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_projected_root_dynamic_task_handle_reinit_object",
            source_relative: "fixtures/codegen/pass/async_program_main_projected_root_dynamic_task_handle_reinit.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_projected_root_const_backed_dynamic_task_handle_reinit_object",
            source_relative: "fixtures/codegen/pass/async_program_main_projected_root_const_backed_dynamic_task_handle_reinit.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_aliased_projected_root_dynamic_task_handle_reinit_object",
            source_relative: "fixtures/codegen/pass/async_program_main_aliased_projected_root_dynamic_task_handle_reinit.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_aliased_projected_root_const_backed_dynamic_task_handle_reinit_object",
            source_relative: "fixtures/codegen/pass/async_program_main_aliased_projected_root_const_backed_dynamic_task_handle_reinit.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_aliased_guard_refined_projected_root_dynamic_task_handle_reinit_object",
            source_relative: "fixtures/codegen/pass/async_program_main_aliased_guard_refined_projected_root_dynamic_task_handle_reinit.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_aliased_guard_refined_const_backed_projected_root_dynamic_task_handle_reinit_object",
            source_relative: "fixtures/codegen/pass/async_program_main_aliased_guard_refined_const_backed_projected_root_dynamic_task_handle_reinit.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_aliased_guard_refined_static_alias_backed_projected_root_dynamic_task_handle_reinit_object",
            source_relative: "fixtures/codegen/pass/async_program_main_aliased_guard_refined_static_alias_backed_projected_root_dynamic_task_handle_reinit.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_aliased_projected_root_task_handle_tuple_repackage_reinit_object",
            source_relative: "fixtures/codegen/pass/async_program_main_aliased_projected_root_task_handle_tuple_repackage_reinit.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_aliased_projected_root_task_handle_struct_repackage_reinit_object",
            source_relative: "fixtures/codegen/pass/async_program_main_aliased_projected_root_task_handle_struct_repackage_reinit.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_aliased_projected_root_task_handle_nested_repackage_reinit_object",
            source_relative: "fixtures/codegen/pass/async_program_main_aliased_projected_root_task_handle_nested_repackage_reinit.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_aliased_projected_root_task_handle_nested_repackage_spawn_object",
            source_relative: "fixtures/codegen/pass/async_program_main_aliased_projected_root_task_handle_nested_repackage_spawn.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_aliased_projected_root_task_handle_array_repackage_spawn_object",
            source_relative: "fixtures/codegen/pass/async_program_main_aliased_projected_root_task_handle_array_repackage_spawn.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_aliased_projected_root_task_handle_nested_array_repackage_spawn_object",
            source_relative: "fixtures/codegen/pass/async_program_main_aliased_projected_root_task_handle_nested_array_repackage_spawn.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_aliased_guard_refined_const_backed_projected_root_task_handle_nested_array_repackage_spawn_object",
            source_relative: "fixtures/codegen/pass/async_program_main_aliased_guard_refined_const_backed_projected_root_task_handle_nested_array_repackage_spawn.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_aliased_projected_root_task_handle_forwarded_nested_array_repackage_spawn_object",
            source_relative: "fixtures/codegen/pass/async_program_main_aliased_projected_root_task_handle_forwarded_nested_array_repackage_spawn.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_aliased_guard_refined_const_backed_projected_root_task_handle_forwarded_nested_array_repackage_spawn_object",
            source_relative: "fixtures/codegen/pass/async_program_main_aliased_guard_refined_const_backed_projected_root_task_handle_forwarded_nested_array_repackage_spawn.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_composed_dynamic_task_handle_nested_array_repackage_spawn_object",
            source_relative: "fixtures/codegen/pass/async_program_main_composed_dynamic_task_handle_nested_array_repackage_spawn.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_alias_sourced_composed_dynamic_task_handle_nested_array_repackage_spawn_object",
            source_relative: "fixtures/codegen/pass/async_program_main_alias_sourced_composed_dynamic_task_handle_nested_array_repackage_spawn.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_guarded_alias_sourced_composed_dynamic_task_handle_nested_array_repackage_spawn_object",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_alias_sourced_composed_dynamic_task_handle_nested_array_repackage_spawn.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn_with_tail_field_object",
            source_relative: "fixtures/codegen/pass/async_program_main_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn_with_tail_field.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_guarded_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn_with_tail_field_object",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn_with_tail_field.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_const_backed_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn_object",
            source_relative: "fixtures/codegen/pass/async_program_main_const_backed_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_guarded_const_backed_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn_object",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_guarded_const_backed_double_root_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn_object",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_double_root_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_guarded_const_backed_double_root_double_source_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn_object",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_double_root_double_source_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_guarded_const_backed_double_root_double_source_row_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn_object",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_double_root_double_source_row_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_guarded_const_backed_double_root_double_source_row_slot_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn_object",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_double_root_double_source_row_slot_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_guarded_const_backed_triple_root_double_source_row_slot_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn_object",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_double_source_row_slot_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t40_triple_source_row_slot_object",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_row_slot_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t41_tail_alias_object",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t42_forwarded_alias_object",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_task_handle_forwarded_alias_nested_array_repackage_spawn.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t43_tail_queued_object",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_queued_spawn.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t44_queue_root_object",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_root_spawn.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t45_queue_root_alias_object",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_root_alias_spawn.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t46_queue_root_chain_object",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_root_chain_spawn.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t47_queue_local_alias_object",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_local_alias_spawn.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t48_queue_local_chain_object",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_local_chain_spawn.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t49_queue_local_forward_object",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_local_forward_spawn.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t50_queue_local_inline_forward_object",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_local_inline_forward_spawn.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t51_bundle_inline_forward_object",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_bundle_inline_forward_spawn.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t52_bundle_slot_inline_forward_object",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_bundle_slot_inline_forward_spawn.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t53_tail_inline_forward_object",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_direct_inline_forward_spawn.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t54_tail_inline_forward_await_object",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_direct_inline_forward_await.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t55_bundle_slot_inline_forward_await_object",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_bundle_slot_inline_forward_await.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t56_bundle_inline_forward_await_object",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_bundle_inline_forward_await.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t57_queue_local_inline_forward_await_object",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_local_inline_forward_await.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t58_queue_local_forward_await_object",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_local_forward_await.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t59_queue_root_inline_forward_await_object",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_root_inline_forward_await.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t60_queue_root_forward_await_object",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_root_forward_await.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t61_queue_root_alias_forward_await_object",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_root_alias_forward_await.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t62_queue_root_chain_forward_await_object",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_root_chain_forward_await.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t63_queue_root_alias_inline_forward_await_object",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_root_alias_inline_forward_await.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t64_queue_root_chain_inline_forward_await_object",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_root_chain_inline_forward_await.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t65_bundle_forward_await_object",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_bundle_forward_await.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t66_bundle_alias_forward_await_object",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_bundle_alias_forward_await.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t67_bundle_chain_forward_await_object",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_bundle_chain_forward_await.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t68_bundle_alias_inline_forward_await_object",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_bundle_alias_inline_forward_await.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t69_bundle_chain_inline_forward_await_object",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_bundle_chain_inline_forward_await.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t70_bundle_forward_spawn_object",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_bundle_forward_spawn.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t71_bundle_alias_forward_spawn_object",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_bundle_alias_forward_spawn.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t72_bundle_chain_forward_spawn_object",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_bundle_chain_forward_spawn.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_aliased_guard_refined_const_backed_projected_root_task_handle_nested_repackage_reinit_object",
            source_relative: "fixtures/codegen/pass/async_program_main_aliased_guard_refined_const_backed_projected_root_task_handle_nested_repackage_reinit.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_aliased_guard_refined_const_backed_projected_root_task_handle_nested_repackage_spawn_object",
            source_relative: "fixtures/codegen/pass/async_program_main_aliased_guard_refined_const_backed_projected_root_task_handle_nested_repackage_spawn.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_aliased_guard_refined_const_backed_projected_root_task_handle_array_repackage_spawn_object",
            source_relative: "fixtures/codegen/pass/async_program_main_aliased_guard_refined_const_backed_projected_root_task_handle_array_repackage_spawn.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_composed_dynamic_task_handle_reinit_object",
            source_relative: "fixtures/codegen/pass/async_program_main_composed_dynamic_task_handle_reinit.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_alias_sourced_composed_dynamic_task_handle_reinit_object",
            source_relative: "fixtures/codegen/pass/async_program_main_alias_sourced_composed_dynamic_task_handle_reinit.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "minimal_build_executable",
            source_relative: "fixtures/codegen/pass/minimal_build.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "extern_c_export_dylib",
            source_relative: "fixtures/codegen/pass/extern_c_export.ql",
            emit: "dylib",
            expected_relative: "tests/codegen/pass/extern_c_export.dylib.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "extern_c_export_dylib_with_header",
            source_relative: "fixtures/codegen/pass/extern_c_export.ql",
            emit: "dylib",
            expected_relative: "tests/codegen/pass/extern_c_export.dylib.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: Some("exports"),
            expected_header_relative: Some("tests/codegen/pass/extern_c_export.h"),
        },
        PassCase {
            name: "ffi_export_async_dylib",
            source_relative: "fixtures/codegen/pass/ffi_export_async.ql",
            emit: "dylib",
            expected_relative: "tests/codegen/pass/extern_c_export.dylib.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "ffi_export_async_dylib_with_header",
            source_relative: "fixtures/codegen/pass/ffi_export_async.ql",
            emit: "dylib",
            expected_relative: "tests/codegen/pass/extern_c_export.dylib.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: Some("exports"),
            expected_header_relative: Some("tests/codegen/pass/ffi_export_async.h"),
        },
        PassCase {
            name: "ffi_export_async_for_await_array_dylib",
            source_relative: "fixtures/codegen/pass/ffi_export_async_for_await_array.ql",
            emit: "dylib",
            expected_relative: "tests/codegen/pass/extern_c_export.dylib.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "ffi_export_async_for_await_tuple_dylib",
            source_relative: "fixtures/codegen/pass/ffi_export_async_for_await_tuple.ql",
            emit: "dylib",
            expected_relative: "tests/codegen/pass/extern_c_export.dylib.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "ffi_export_async_task_tuple_for_await_dylib",
            source_relative: "fixtures/codegen/pass/ffi_export_async_task_tuple_for_await.ql",
            emit: "dylib",
            expected_relative: "tests/codegen/pass/extern_c_export.dylib.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "ffi_export_async_task_array_for_await_dylib",
            source_relative: "fixtures/codegen/pass/ffi_export_async_task_array_for_await.ql",
            emit: "dylib",
            expected_relative: "tests/codegen/pass/extern_c_export.dylib.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "ffi_export_async_aggregate_await_families_dylib",
            source_relative: "fixtures/codegen/pass/ffi_export_async_aggregate_await_families.ql",
            emit: "dylib",
            expected_relative: "tests/codegen/pass/extern_c_export.dylib.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "ffi_export_async_aggregate_param_result_families_dylib",
            source_relative: "fixtures/codegen/pass/ffi_export_async_aggregate_param_result_families.ql",
            emit: "dylib",
            expected_relative: "tests/codegen/pass/extern_c_export.dylib.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "ffi_export_async_match_families_dylib",
            source_relative: "fixtures/codegen/pass/ffi_export_async_match_families.ql",
            emit: "dylib",
            expected_relative: "tests/codegen/pass/extern_c_export.dylib.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "ffi_export_async_task_handle_payload_families_dylib",
            source_relative: "fixtures/codegen/pass/ffi_export_async_task_handle_payload_families.ql",
            emit: "dylib",
            expected_relative: "tests/codegen/pass/extern_c_export.dylib.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "ffi_export_async_task_handle_flow_families_dylib",
            source_relative: "fixtures/codegen/pass/ffi_export_async_task_handle_flow_families.ql",
            emit: "dylib",
            expected_relative: "tests/codegen/pass/extern_c_export.dylib.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "ffi_export_async_dynamic_task_handle_paths_dylib",
            source_relative: "fixtures/codegen/pass/ffi_export_async_dynamic_task_handle_paths.ql",
            emit: "dylib",
            expected_relative: "tests/codegen/pass/extern_c_export.dylib.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "ffi_export_async_aliased_projected_root_repackage_families_dylib",
            source_relative: "fixtures/codegen/pass/ffi_export_async_aliased_projected_root_repackage_families.ql",
            emit: "dylib",
            expected_relative: "tests/codegen/pass/extern_c_export.dylib.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "ffi_export_async_aliased_projected_root_spawn_families_dylib",
            source_relative: "fixtures/codegen/pass/ffi_export_async_aliased_projected_root_spawn_families.ql",
            emit: "dylib",
            expected_relative: "tests/codegen/pass/extern_c_export.dylib.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "ffi_export_async_guard_refined_dynamic_path_families_dylib",
            source_relative: "fixtures/codegen/pass/ffi_export_async_guard_refined_dynamic_path_families.ql",
            emit: "dylib",
            expected_relative: "tests/codegen/pass/extern_c_export.dylib.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "ffi_export_async_projected_reinit_families_dylib",
            source_relative: "fixtures/codegen/pass/ffi_export_async_projected_reinit_families.ql",
            emit: "dylib",
            expected_relative: "tests/codegen/pass/extern_c_export.dylib.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "ffi_export_async_task_handle_consume_families_dylib",
            source_relative: "fixtures/codegen/pass/ffi_export_async_task_handle_consume_families.ql",
            emit: "dylib",
            expected_relative: "tests/codegen/pass/extern_c_export.dylib.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "ffi_export_async_inline_without_parens_for_await_dylib",
            source_relative: "fixtures/codegen/pass/ffi_export_async_inline_without_parens_for_await.ql",
            emit: "dylib",
            expected_relative: "tests/codegen/pass/extern_c_export.dylib.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "ffi_export_async_inline_for_await_dylib",
            source_relative: "fixtures/codegen/pass/ffi_export_async_inline_for_await.ql",
            emit: "dylib",
            expected_relative: "tests/codegen/pass/extern_c_export.dylib.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "ffi_export_async_import_alias_awaited_for_await_dylib",
            source_relative: "fixtures/codegen/pass/ffi_export_async_import_alias_awaited_for_await.ql",
            emit: "dylib",
            expected_relative: "tests/codegen/pass/extern_c_export.dylib.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "ffi_export_async_import_alias_for_await_dylib",
            source_relative: "fixtures/codegen/pass/ffi_export_async_import_alias_for_await.ql",
            emit: "dylib",
            expected_relative: "tests/codegen/pass/extern_c_export.dylib.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "ffi_export_async_nested_call_root_for_await_dylib",
            source_relative: "fixtures/codegen/pass/ffi_export_async_nested_call_root_for_await.ql",
            emit: "dylib",
            expected_relative: "tests/codegen/pass/extern_c_export.dylib.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "ffi_export_async_awaited_projected_for_await_dylib",
            source_relative: "fixtures/codegen/pass/ffi_export_async_awaited_projected_for_await.ql",
            emit: "dylib",
            expected_relative: "tests/codegen/pass/extern_c_export.dylib.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "ffi_export_async_call_root_for_await_dylib",
            source_relative: "fixtures/codegen/pass/ffi_export_async_call_root_for_await.ql",
            emit: "dylib",
            expected_relative: "tests/codegen/pass/extern_c_export.dylib.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "ffi_export_async_projected_for_await_dylib",
            source_relative: "fixtures/codegen/pass/ffi_export_async_projected_for_await.ql",
            emit: "dylib",
            expected_relative: "tests/codegen/pass/extern_c_export.dylib.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "minimal_library_staticlib",
            source_relative: "fixtures/codegen/pass/minimal_library.ql",
            emit: "staticlib",
            expected_relative: "tests/codegen/pass/minimal_library.staticlib.txt",
            mock_compiler: true,
            mock_archiver: true,
            archiver_style: Some(current_archiver_style()),
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "dynamic_array_assignment_staticlib",
            source_relative: "fixtures/codegen/pass/dynamic_array_assignment.ql",
            emit: "staticlib",
            expected_relative: "tests/codegen/pass/minimal_library.staticlib.txt",
            mock_compiler: true,
            mock_archiver: true,
            archiver_style: Some(current_archiver_style()),
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "dynamic_nested_array_assignment_staticlib",
            source_relative: "fixtures/codegen/pass/dynamic_nested_array_assignment.ql",
            emit: "staticlib",
            expected_relative: "tests/codegen/pass/minimal_library.staticlib.txt",
            mock_compiler: true,
            mock_archiver: true,
            archiver_style: Some(current_archiver_style()),
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_dynamic_task_array_assignment_staticlib",
            source_relative: "fixtures/codegen/pass/async_dynamic_task_array_assignment.ql",
            emit: "staticlib",
            expected_relative: "tests/codegen/pass/minimal_library.staticlib.txt",
            mock_compiler: true,
            mock_archiver: true,
            archiver_style: Some(current_archiver_style()),
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_library_aggregate_param_result_families_staticlib",
            source_relative: "fixtures/codegen/pass/async_library_aggregate_param_result_families.ql",
            emit: "staticlib",
            expected_relative: "tests/codegen/pass/minimal_library.staticlib.txt",
            mock_compiler: true,
            mock_archiver: true,
            archiver_style: Some(current_archiver_style()),
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_library_match_families_staticlib",
            source_relative: "fixtures/codegen/pass/async_library_match_families.ql",
            emit: "staticlib",
            expected_relative: "tests/codegen/pass/minimal_library.staticlib.txt",
            mock_compiler: true,
            mock_archiver: true,
            archiver_style: Some(current_archiver_style()),
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_library_spawn_staticlib",
            source_relative: "fixtures/codegen/pass/async_library_spawn.ql",
            emit: "staticlib",
            expected_relative: "tests/codegen/pass/minimal_library.staticlib.txt",
            mock_compiler: true,
            mock_archiver: true,
            archiver_style: Some(current_archiver_style()),
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_library_for_await_array_staticlib",
            source_relative: "fixtures/codegen/pass/async_library_for_await_array.ql",
            emit: "staticlib",
            expected_relative: "tests/codegen/pass/minimal_library.staticlib.txt",
            mock_compiler: true,
            mock_archiver: true,
            archiver_style: Some(current_archiver_style()),
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_library_for_await_tuple_staticlib",
            source_relative: "fixtures/codegen/pass/async_library_for_await_tuple.ql",
            emit: "staticlib",
            expected_relative: "tests/codegen/pass/minimal_library.staticlib.txt",
            mock_compiler: true,
            mock_archiver: true,
            archiver_style: Some(current_archiver_style()),
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_library_task_array_for_await_staticlib",
            source_relative: "fixtures/codegen/pass/async_library_task_array_for_await.ql",
            emit: "staticlib",
            expected_relative: "tests/codegen/pass/minimal_library.staticlib.txt",
            mock_compiler: true,
            mock_archiver: true,
            archiver_style: Some(current_archiver_style()),
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_library_task_tuple_for_await_staticlib",
            source_relative: "fixtures/codegen/pass/async_library_task_tuple_for_await.ql",
            emit: "staticlib",
            expected_relative: "tests/codegen/pass/minimal_library.staticlib.txt",
            mock_compiler: true,
            mock_archiver: true,
            archiver_style: Some(current_archiver_style()),
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_library_inline_without_parens_for_await_staticlib",
            source_relative: "fixtures/codegen/pass/async_library_inline_without_parens_for_await.ql",
            emit: "staticlib",
            expected_relative: "tests/codegen/pass/minimal_library.staticlib.txt",
            mock_compiler: true,
            mock_archiver: true,
            archiver_style: Some(current_archiver_style()),
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_library_inline_for_await_staticlib",
            source_relative: "fixtures/codegen/pass/async_library_inline_for_await.ql",
            emit: "staticlib",
            expected_relative: "tests/codegen/pass/minimal_library.staticlib.txt",
            mock_compiler: true,
            mock_archiver: true,
            archiver_style: Some(current_archiver_style()),
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_library_import_alias_awaited_for_await_staticlib",
            source_relative: "fixtures/codegen/pass/async_library_import_alias_awaited_for_await.ql",
            emit: "staticlib",
            expected_relative: "tests/codegen/pass/minimal_library.staticlib.txt",
            mock_compiler: true,
            mock_archiver: true,
            archiver_style: Some(current_archiver_style()),
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_library_import_alias_for_await_staticlib",
            source_relative: "fixtures/codegen/pass/async_library_import_alias_for_await.ql",
            emit: "staticlib",
            expected_relative: "tests/codegen/pass/minimal_library.staticlib.txt",
            mock_compiler: true,
            mock_archiver: true,
            archiver_style: Some(current_archiver_style()),
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_library_nested_call_root_for_await_staticlib",
            source_relative: "fixtures/codegen/pass/async_library_nested_call_root_for_await.ql",
            emit: "staticlib",
            expected_relative: "tests/codegen/pass/minimal_library.staticlib.txt",
            mock_compiler: true,
            mock_archiver: true,
            archiver_style: Some(current_archiver_style()),
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_library_awaited_projected_for_await_staticlib",
            source_relative: "fixtures/codegen/pass/async_library_awaited_projected_for_await.ql",
            emit: "staticlib",
            expected_relative: "tests/codegen/pass/minimal_library.staticlib.txt",
            mock_compiler: true,
            mock_archiver: true,
            archiver_style: Some(current_archiver_style()),
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_library_call_root_for_await_staticlib",
            source_relative: "fixtures/codegen/pass/async_library_call_root_for_await.ql",
            emit: "staticlib",
            expected_relative: "tests/codegen/pass/minimal_library.staticlib.txt",
            mock_compiler: true,
            mock_archiver: true,
            archiver_style: Some(current_archiver_style()),
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_library_projected_for_await_staticlib",
            source_relative: "fixtures/codegen/pass/async_library_projected_for_await.ql",
            emit: "staticlib",
            expected_relative: "tests/codegen/pass/minimal_library.staticlib.txt",
            mock_compiler: true,
            mock_archiver: true,
            archiver_style: Some(current_archiver_style()),
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_library_aggregate_await_families_staticlib",
            source_relative: "fixtures/codegen/pass/async_library_aggregate_await_families.ql",
            emit: "staticlib",
            expected_relative: "tests/codegen/pass/minimal_library.staticlib.txt",
            mock_compiler: true,
            mock_archiver: true,
            archiver_style: Some(current_archiver_style()),
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_library_task_handle_payload_families_staticlib",
            source_relative: "fixtures/codegen/pass/async_library_task_handle_payload_families.ql",
            emit: "staticlib",
            expected_relative: "tests/codegen/pass/minimal_library.staticlib.txt",
            mock_compiler: true,
            mock_archiver: true,
            archiver_style: Some(current_archiver_style()),
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_library_task_handle_flow_families_staticlib",
            source_relative: "fixtures/codegen/pass/async_library_task_handle_flow_families.ql",
            emit: "staticlib",
            expected_relative: "tests/codegen/pass/minimal_library.staticlib.txt",
            mock_compiler: true,
            mock_archiver: true,
            archiver_style: Some(current_archiver_style()),
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_library_dynamic_task_handle_paths_staticlib",
            source_relative: "fixtures/codegen/pass/async_library_dynamic_task_handle_paths.ql",
            emit: "staticlib",
            expected_relative: "tests/codegen/pass/minimal_library.staticlib.txt",
            mock_compiler: true,
            mock_archiver: true,
            archiver_style: Some(current_archiver_style()),
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_library_aliased_projected_root_repackage_families_staticlib",
            source_relative: "fixtures/codegen/pass/async_library_aliased_projected_root_repackage_families.ql",
            emit: "staticlib",
            expected_relative: "tests/codegen/pass/minimal_library.staticlib.txt",
            mock_compiler: true,
            mock_archiver: true,
            archiver_style: Some(current_archiver_style()),
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_library_aliased_projected_root_spawn_families_staticlib",
            source_relative: "fixtures/codegen/pass/async_library_aliased_projected_root_spawn_families.ql",
            emit: "staticlib",
            expected_relative: "tests/codegen/pass/minimal_library.staticlib.txt",
            mock_compiler: true,
            mock_archiver: true,
            archiver_style: Some(current_archiver_style()),
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_library_guard_refined_dynamic_path_families_staticlib",
            source_relative: "fixtures/codegen/pass/async_library_guard_refined_dynamic_path_families.ql",
            emit: "staticlib",
            expected_relative: "tests/codegen/pass/minimal_library.staticlib.txt",
            mock_compiler: true,
            mock_archiver: true,
            archiver_style: Some(current_archiver_style()),
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_library_projected_reinit_families_staticlib",
            source_relative: "fixtures/codegen/pass/async_library_projected_reinit_families.ql",
            emit: "staticlib",
            expected_relative: "tests/codegen/pass/minimal_library.staticlib.txt",
            mock_compiler: true,
            mock_archiver: true,
            archiver_style: Some(current_archiver_style()),
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_library_task_handle_consume_families_staticlib",
            source_relative: "fixtures/codegen/pass/async_library_task_handle_consume_families.ql",
            emit: "staticlib",
            expected_relative: "tests/codegen/pass/minimal_library.staticlib.txt",
            mock_compiler: true,
            mock_archiver: true,
            archiver_style: Some(current_archiver_style()),
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_library_spawn_zero_sized_aggregate_result_staticlib",
            source_relative: "fixtures/codegen/pass/async_library_spawn_zero_sized_aggregate_result.ql",
            emit: "staticlib",
            expected_relative: "tests/codegen/pass/minimal_library.staticlib.txt",
            mock_compiler: true,
            mock_archiver: true,
            archiver_style: Some(current_archiver_style()),
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "ffi_export_async_staticlib_with_header",
            source_relative: "fixtures/codegen/pass/ffi_export_async.ql",
            emit: "staticlib",
            expected_relative: "tests/codegen/pass/minimal_library.staticlib.txt",
            mock_compiler: true,
            mock_archiver: true,
            archiver_style: Some(current_archiver_style()),
            header_surface: Some("exports"),
            expected_header_relative: Some("tests/codegen/pass/ffi_export_async.h"),
        },
        PassCase {
            name: "extern_c_library_staticlib",
            source_relative: "fixtures/codegen/pass/extern_c_library.ql",
            emit: "staticlib",
            expected_relative: "tests/codegen/pass/extern_c_library.staticlib.txt",
            mock_compiler: true,
            mock_archiver: true,
            archiver_style: Some(current_archiver_style()),
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "extern_c_library_staticlib_with_import_header",
            source_relative: "fixtures/codegen/pass/extern_c_library.ql",
            emit: "staticlib",
            expected_relative: "tests/codegen/pass/extern_c_library.staticlib.txt",
            mock_compiler: true,
            mock_archiver: true,
            archiver_style: Some(current_archiver_style()),
            header_surface: Some("imports"),
            expected_header_relative: Some("tests/codegen/pass/extern_c_library.imports.h"),
        },
        PassCase {
            name: "extern_c_import_top_level_staticlib_with_both_header",
            source_relative: "tests/ffi/pass/extern_c_import_top_level.ql",
            emit: "staticlib",
            expected_relative: "tests/codegen/pass/extern_c_import_top_level.staticlib.txt",
            mock_compiler: true,
            mock_archiver: true,
            archiver_style: Some(current_archiver_style()),
            header_surface: Some("both"),
            expected_header_relative: Some("tests/codegen/pass/extern_c_import_top_level.ffi.h"),
        },
        PassCase {
            name: "extern_c_top_level_library_staticlib",
            source_relative: "fixtures/codegen/pass/extern_c_top_level_library.ql",
            emit: "staticlib",
            expected_relative: "tests/codegen/pass/extern_c_top_level_library.staticlib.txt",
            mock_compiler: true,
            mock_archiver: true,
            archiver_style: Some(current_archiver_style()),
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_program_main.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_program_main.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_spawn_bound_task_handle_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_program_main_spawn_bound_task_handle.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_program_main_spawn_bound_task_handle.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_for_await_array_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_program_main_for_await_array.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_program_main_for_await_array.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_guard_refined_dynamic_task_handle_reinit_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_program_main_guard_refined_dynamic_task_handle_reinit.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_program_main_guard_refined_dynamic_task_handle_reinit.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_guard_refined_projected_dynamic_task_handle_reinit_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_program_main_guard_refined_projected_dynamic_task_handle_reinit.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_program_main_guard_refined_projected_dynamic_task_handle_reinit.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_projected_root_dynamic_task_handle_reinit_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_program_main_projected_root_dynamic_task_handle_reinit.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_program_main_projected_root_dynamic_task_handle_reinit.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_projected_root_const_backed_dynamic_task_handle_reinit_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_program_main_projected_root_const_backed_dynamic_task_handle_reinit.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_program_main_projected_root_const_backed_dynamic_task_handle_reinit.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_aliased_projected_root_dynamic_task_handle_reinit_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_program_main_aliased_projected_root_dynamic_task_handle_reinit.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_program_main_aliased_projected_root_dynamic_task_handle_reinit.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_aliased_projected_root_const_backed_dynamic_task_handle_reinit_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_program_main_aliased_projected_root_const_backed_dynamic_task_handle_reinit.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_program_main_aliased_projected_root_const_backed_dynamic_task_handle_reinit.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_aliased_guard_refined_projected_root_dynamic_task_handle_reinit_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_program_main_aliased_guard_refined_projected_root_dynamic_task_handle_reinit.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_program_main_aliased_guard_refined_projected_root_dynamic_task_handle_reinit.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_aliased_guard_refined_const_backed_projected_root_dynamic_task_handle_reinit_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_program_main_aliased_guard_refined_const_backed_projected_root_dynamic_task_handle_reinit.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_program_main_aliased_guard_refined_const_backed_projected_root_dynamic_task_handle_reinit.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_aliased_guard_refined_static_alias_backed_projected_root_dynamic_task_handle_reinit_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_program_main_aliased_guard_refined_static_alias_backed_projected_root_dynamic_task_handle_reinit.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_program_main_aliased_guard_refined_static_alias_backed_projected_root_dynamic_task_handle_reinit.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_aliased_projected_root_task_handle_tuple_repackage_reinit_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_program_main_aliased_projected_root_task_handle_tuple_repackage_reinit.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_program_main_aliased_projected_root_task_handle_tuple_repackage_reinit.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_aliased_projected_root_task_handle_struct_repackage_reinit_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_program_main_aliased_projected_root_task_handle_struct_repackage_reinit.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_program_main_aliased_projected_root_task_handle_struct_repackage_reinit.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_aliased_projected_root_task_handle_nested_repackage_reinit_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_program_main_aliased_projected_root_task_handle_nested_repackage_reinit.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_program_main_aliased_projected_root_task_handle_nested_repackage_reinit.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_aliased_projected_root_task_handle_nested_repackage_spawn_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_program_main_aliased_projected_root_task_handle_nested_repackage_spawn.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_program_main_aliased_projected_root_task_handle_nested_repackage_spawn.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_aliased_projected_root_task_handle_array_repackage_spawn_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_program_main_aliased_projected_root_task_handle_array_repackage_spawn.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_program_main_aliased_projected_root_task_handle_array_repackage_spawn.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_aliased_projected_root_task_handle_nested_array_repackage_spawn_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_program_main_aliased_projected_root_task_handle_nested_array_repackage_spawn.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_program_main_aliased_projected_root_task_handle_nested_array_repackage_spawn.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_aliased_guard_refined_const_backed_projected_root_task_handle_nested_array_repackage_spawn_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_program_main_aliased_guard_refined_const_backed_projected_root_task_handle_nested_array_repackage_spawn.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_program_main_aliased_guard_refined_const_backed_projected_root_task_handle_nested_array_repackage_spawn.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_aliased_projected_root_task_handle_forwarded_nested_array_repackage_spawn_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_program_main_aliased_projected_root_task_handle_forwarded_nested_array_repackage_spawn.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_program_main_aliased_projected_root_task_handle_forwarded_nested_array_repackage_spawn.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_aliased_guard_refined_const_backed_projected_root_task_handle_forwarded_nested_array_repackage_spawn_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_program_main_aliased_guard_refined_const_backed_projected_root_task_handle_forwarded_nested_array_repackage_spawn.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_program_main_aliased_guard_refined_const_backed_projected_root_task_handle_forwarded_nested_array_repackage_spawn.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_composed_dynamic_task_handle_nested_array_repackage_spawn_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_program_main_composed_dynamic_task_handle_nested_array_repackage_spawn.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_program_main_composed_dynamic_task_handle_nested_array_repackage_spawn.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_alias_sourced_composed_dynamic_task_handle_nested_array_repackage_spawn_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_program_main_alias_sourced_composed_dynamic_task_handle_nested_array_repackage_spawn.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_program_main_alias_sourced_composed_dynamic_task_handle_nested_array_repackage_spawn.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_guarded_alias_sourced_composed_dynamic_task_handle_nested_array_repackage_spawn_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_alias_sourced_composed_dynamic_task_handle_nested_array_repackage_spawn.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_program_main_guarded_alias_sourced_composed_dynamic_task_handle_nested_array_repackage_spawn.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn_with_tail_field_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_program_main_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn_with_tail_field.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_program_main_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn_with_tail_field.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_guarded_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn_with_tail_field_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn_with_tail_field.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_program_main_guarded_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn_with_tail_field.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_const_backed_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_program_main_const_backed_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_program_main_const_backed_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_guarded_const_backed_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_program_main_guarded_const_backed_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_guarded_const_backed_double_root_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_double_root_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_program_main_guarded_const_backed_double_root_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_guarded_const_backed_double_root_double_source_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_double_root_double_source_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_program_main_guarded_const_backed_double_root_double_source_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_guarded_const_backed_double_root_double_source_row_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_double_root_double_source_row_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_program_main_guarded_const_backed_double_root_double_source_row_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_guarded_const_backed_double_root_double_source_row_slot_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_double_root_double_source_row_slot_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_program_main_guarded_const_backed_double_root_double_source_row_slot_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_guarded_const_backed_triple_root_double_source_row_slot_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_double_source_row_slot_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_program_main_guarded_const_backed_triple_root_double_source_row_slot_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t40_triple_source_row_slot_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_row_slot_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_row_slot_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t41_tail_alias_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t42_forwarded_alias_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_task_handle_forwarded_alias_nested_array_repackage_spawn.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_task_handle_forwarded_alias_nested_array_repackage_spawn.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t43_tail_queued_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_queued_spawn.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_queued_spawn.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t44_queue_root_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_root_spawn.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_root_spawn.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t45_queue_root_alias_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_root_alias_spawn.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_root_alias_spawn.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t46_queue_root_chain_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_root_chain_spawn.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_root_chain_spawn.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t47_queue_local_alias_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_local_alias_spawn.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_local_alias_spawn.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t48_queue_local_chain_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_local_chain_spawn.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_local_chain_spawn.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t49_queue_local_forward_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_local_forward_spawn.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_local_forward_spawn.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t50_queue_local_inline_forward_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_local_inline_forward_spawn.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_local_inline_forward_spawn.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t51_bundle_inline_forward_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_bundle_inline_forward_spawn.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_bundle_inline_forward_spawn.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t52_bundle_slot_inline_forward_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_bundle_slot_inline_forward_spawn.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_bundle_slot_inline_forward_spawn.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t53_tail_inline_forward_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_direct_inline_forward_spawn.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_direct_inline_forward_spawn.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t54_tail_inline_forward_await_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_direct_inline_forward_await.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_direct_inline_forward_await.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t55_bundle_slot_inline_forward_await_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_bundle_slot_inline_forward_await.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_bundle_slot_inline_forward_await.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t56_bundle_inline_forward_await_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_bundle_inline_forward_await.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_bundle_inline_forward_await.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t57_queue_local_inline_forward_await_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_local_inline_forward_await.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_local_inline_forward_await.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t58_queue_local_forward_await_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_local_forward_await.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_local_forward_await.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t59_queue_root_inline_forward_await_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_root_inline_forward_await.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_root_inline_forward_await.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t60_queue_root_forward_await_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_root_forward_await.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_root_forward_await.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t61_queue_root_alias_forward_await_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_root_alias_forward_await.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_root_alias_forward_await.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t62_queue_root_chain_forward_await_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_root_chain_forward_await.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_root_chain_forward_await.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t63_queue_root_alias_inline_forward_await_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_root_alias_inline_forward_await.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_root_alias_inline_forward_await.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t64_queue_root_chain_inline_forward_await_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_root_chain_inline_forward_await.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_root_chain_inline_forward_await.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t65_bundle_forward_await_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_bundle_forward_await.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_bundle_forward_await.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t66_bundle_alias_forward_await_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_bundle_alias_forward_await.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_bundle_alias_forward_await.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t67_bundle_chain_forward_await_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_bundle_chain_forward_await.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_bundle_chain_forward_await.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t68_bundle_alias_inline_forward_await_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_bundle_alias_inline_forward_await.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_bundle_alias_inline_forward_await.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t69_bundle_chain_inline_forward_await_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_bundle_chain_inline_forward_await.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_bundle_chain_inline_forward_await.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t70_bundle_forward_spawn_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_bundle_forward_spawn.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_bundle_forward_spawn.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t71_bundle_alias_forward_spawn_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_bundle_alias_forward_spawn.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_bundle_alias_forward_spawn.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t72_bundle_chain_forward_spawn_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_bundle_chain_forward_spawn.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_bundle_chain_forward_spawn.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_aliased_guard_refined_const_backed_projected_root_task_handle_nested_repackage_reinit_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_program_main_aliased_guard_refined_const_backed_projected_root_task_handle_nested_repackage_reinit.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_program_main_aliased_guard_refined_const_backed_projected_root_task_handle_nested_repackage_reinit.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_aliased_guard_refined_const_backed_projected_root_task_handle_nested_repackage_spawn_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_program_main_aliased_guard_refined_const_backed_projected_root_task_handle_nested_repackage_spawn.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_program_main_aliased_guard_refined_const_backed_projected_root_task_handle_nested_repackage_spawn.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_aliased_guard_refined_const_backed_projected_root_task_handle_array_repackage_spawn_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_program_main_aliased_guard_refined_const_backed_projected_root_task_handle_array_repackage_spawn.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_program_main_aliased_guard_refined_const_backed_projected_root_task_handle_array_repackage_spawn.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_composed_dynamic_task_handle_reinit_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_program_main_composed_dynamic_task_handle_reinit.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_program_main_composed_dynamic_task_handle_reinit.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_alias_sourced_composed_dynamic_task_handle_reinit_llvm_ir",
            source_relative: "fixtures/codegen/pass/async_program_main_alias_sourced_composed_dynamic_task_handle_reinit.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/async_program_main_alias_sourced_composed_dynamic_task_handle_reinit.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_exe",
            source_relative: "fixtures/codegen/pass/async_program_main.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "for_array_exe",
            source_relative: "fixtures/codegen/pass/for_array.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "bool_match_exe",
            source_relative: "fixtures/codegen/pass/bool_match.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "integer_match_exe",
            source_relative: "fixtures/codegen/pass/integer_match.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "integer_dynamic_guard_match_exe",
            source_relative: "fixtures/codegen/pass/integer_dynamic_guard_match.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "integer_comparison_guard_match_exe",
            source_relative: "fixtures/codegen/pass/integer_comparison_guard_match.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "projected_integer_comparison_guard_match_exe",
            source_relative: "fixtures/codegen/pass/projected_integer_comparison_guard_match.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "const_projected_integer_comparison_guard_match_exe",
            source_relative: "fixtures/codegen/pass/const_projected_integer_comparison_guard_match.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "integer_dynamic_guard_catch_all_match_exe",
            source_relative: "fixtures/codegen/pass/integer_dynamic_guard_catch_all_match.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "integer_match_binding_exe",
            source_relative: "fixtures/codegen/pass/integer_match_binding.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "literal_guard_match_exe",
            source_relative: "fixtures/codegen/pass/literal_guard_match.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "const_guard_match_exe",
            source_relative: "fixtures/codegen/pass/const_guard_match.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "bool_dynamic_guard_match_exe",
            source_relative: "fixtures/codegen/pass/bool_dynamic_guard_match.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "negated_bool_guard_match_exe",
            source_relative: "fixtures/codegen/pass/negated_bool_guard_match.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "alias_const_guard_match_exe",
            source_relative: "fixtures/codegen/pass/alias_const_guard_match.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "match_guard_direct_calls_exe",
            source_relative: "fixtures/codegen/pass/match_guard_direct_calls.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "match_guard_call_projection_roots_exe",
            source_relative: "fixtures/codegen/pass/match_guard_call_projection_roots.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "match_guard_aggregate_call_args_exe",
            source_relative: "fixtures/codegen/pass/match_guard_aggregate_call_args.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "match_guard_inline_aggregate_call_args_exe",
            source_relative: "fixtures/codegen/pass/match_guard_inline_aggregate_call_args.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "match_guard_inline_projection_roots_exe",
            source_relative: "fixtures/codegen/pass/match_guard_inline_projection_roots.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "match_guard_item_backed_inline_combos_exe",
            source_relative: "fixtures/codegen/pass/match_guard_item_backed_inline_combos.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "match_guard_call_backed_combos_exe",
            source_relative: "fixtures/codegen/pass/match_guard_call_backed_combos.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "match_guard_call_root_nested_runtime_projection_exe",
            source_relative: "fixtures/codegen/pass/match_guard_call_root_nested_runtime_projection.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "match_guard_nested_call_root_inline_combos_exe",
            source_relative: "fixtures/codegen/pass/match_guard_nested_call_root_inline_combos.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "match_guard_item_backed_nested_call_root_combos_exe",
            source_relative: "fixtures/codegen/pass/match_guard_item_backed_nested_call_root_combos.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "match_guard_call_backed_nested_call_root_combos_exe",
            source_relative: "fixtures/codegen/pass/match_guard_call_backed_nested_call_root_combos.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "match_guard_alias_backed_nested_call_root_combos_exe",
            source_relative: "fixtures/codegen/pass/match_guard_alias_backed_nested_call_root_combos.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "match_guard_binding_backed_nested_call_root_combos_exe",
            source_relative: "fixtures/codegen/pass/match_guard_binding_backed_nested_call_root_combos.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "match_guard_projection_backed_nested_call_root_combos_exe",
            source_relative: "fixtures/codegen/pass/match_guard_projection_backed_nested_call_root_combos.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "for_call_root_fixed_shapes_exe",
            source_relative: "fixtures/codegen/pass/for_call_root_fixed_shapes.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "import_alias_call_root_fixed_shapes_exe",
            source_relative: "fixtures/codegen/pass/import_alias_call_root_fixed_shapes.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "nested_call_root_fixed_shapes_exe",
            source_relative: "fixtures/codegen/pass/nested_call_root_fixed_shapes.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "import_alias_nested_call_root_fixed_shapes_exe",
            source_relative: "fixtures/codegen/pass/import_alias_nested_call_root_fixed_shapes.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_for_await_array_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_for_await_array.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_nested_task_handle_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_nested_task_handle.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_task_handle_tuple_payload_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_task_handle_tuple_payload.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_task_handle_array_payload_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_task_handle_array_payload.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_nested_aggregate_task_handle_payload_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_nested_aggregate_task_handle_payload.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_helper_task_handle_flows_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_helper_task_handle_flows.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_zero_sized_helper_task_handle_flows_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_zero_sized_helper_task_handle_flows.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_local_return_task_handle_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_local_return_task_handle.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_direct_handle_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_direct_handle.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_spawn_bound_task_handle_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_spawn_bound_task_handle.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_local_return_zero_sized_task_handle_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_local_return_zero_sized_task_handle.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_zero_sized_aggregate_results_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_zero_sized_aggregate_results.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_spawn_zero_sized_aggregate_result_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_spawn_zero_sized_aggregate_result.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_aggregate_results_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_aggregate_results.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_spawned_aggregate_results_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_spawned_aggregate_results.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_recursive_aggregate_results_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_recursive_aggregate_results.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_spawned_recursive_aggregate_results_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_spawned_recursive_aggregate_results.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_recursive_aggregate_params_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_recursive_aggregate_params.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_spawned_recursive_aggregate_params_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_spawned_recursive_aggregate_params.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_zero_sized_aggregate_params_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_zero_sized_aggregate_params.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_spawned_zero_sized_aggregate_params_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_spawned_zero_sized_aggregate_params.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_projected_task_handle_awaits_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_projected_task_handle_awaits.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_projected_task_handle_spawns_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_projected_task_handle_spawns.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_projected_task_handle_reinit_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_projected_task_handle_reinit.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_projected_task_handle_conditional_reinit_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_projected_task_handle_conditional_reinit.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_zero_sized_nested_task_handle_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_zero_sized_nested_task_handle.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_zero_sized_struct_task_handle_payload_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_zero_sized_struct_task_handle_payload.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_zero_sized_projected_task_handle_awaits_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_zero_sized_projected_task_handle_awaits.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_zero_sized_projected_task_handle_spawns_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_zero_sized_projected_task_handle_spawns.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_zero_sized_projected_task_handle_reinit_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_zero_sized_projected_task_handle_reinit.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_zero_sized_projected_task_handle_conditional_reinit_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_zero_sized_projected_task_handle_conditional_reinit.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_branch_spawned_reinit_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_branch_spawned_reinit.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_zero_sized_branch_spawned_reinit_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_zero_sized_branch_spawned_reinit.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_zero_sized_reverse_branch_spawned_reinit_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_zero_sized_reverse_branch_spawned_reinit.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_conditional_async_call_spawns_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_conditional_async_call_spawns.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_zero_sized_conditional_async_call_spawns_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_zero_sized_conditional_async_call_spawns.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_conditional_helper_task_handle_spawns_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_conditional_helper_task_handle_spawns.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_zero_sized_conditional_helper_task_handle_spawns_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_zero_sized_conditional_helper_task_handle_spawns.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
    ];
    pass_cases.extend(dynamic_task_handle_pass_cases());
    pass_cases.extend(projected_dynamic_task_handle_pass_cases());

    let mut fail_cases = vec![
        FailCase {
            name: "unsupported_capturing_closure_build",
            source_relative: "tests/codegen/fail/unsupported_capturing_closure_build.ql",
            emit: "llvm-ir",
            expected_stderr_relative: "tests/codegen/fail/unsupported_capturing_closure_build.stderr",
            extra_args: &[],
        },
        FailCase {
            name: "unsupported_extern_rust_abi_build",
            source_relative: "tests/codegen/fail/unsupported_extern_rust_abi_build.ql",
            emit: "llvm-ir",
            expected_stderr_relative: "tests/codegen/fail/unsupported_extern_rust_abi_build.stderr",
            extra_args: &[],
        },
        FailCase {
            name: "unsupported_extern_rust_abi_definition_build",
            source_relative: "tests/codegen/fail/unsupported_extern_rust_abi_definition_build.ql",
            emit: "llvm-ir",
            expected_stderr_relative: "tests/codegen/fail/unsupported_extern_rust_abi_definition_build.stderr",
            extra_args: &[],
        },
        FailCase {
            name: "unsupported_empty_array_without_expected_build",
            source_relative: "tests/codegen/fail/unsupported_empty_array_without_expected_build.ql",
            emit: "llvm-ir",
            expected_stderr_relative: "tests/codegen/fail/unsupported_empty_array_without_expected_build.stderr",
            extra_args: &[],
        },
        FailCase {
            name: "unsupported_cleanup_capturing_closure_value_build",
            source_relative: "tests/codegen/fail/unsupported_cleanup_capturing_closure_value_build.ql",
            emit: "llvm-ir",
            expected_stderr_relative: "tests/codegen/fail/unsupported_cleanup_capturing_closure_value_build.stderr",
            extra_args: &[],
        },
        FailCase {
            name: "unsupported_for_build",
            source_relative: "tests/codegen/fail/unsupported_for_build.ql",
            emit: "llvm-ir",
            expected_stderr_relative: "tests/codegen/fail/unsupported_for_build.stderr",
            extra_args: &[],
        },
        FailCase {
            name: "unsupported_cleanup_for_build",
            source_relative: "tests/codegen/fail/unsupported_cleanup_for_build.ql",
            emit: "llvm-ir",
            expected_stderr_relative: "tests/codegen/fail/unsupported_cleanup_for_build.stderr",
            extra_args: &[],
        },
        FailCase {
            name: "unsupported_async_generic_main_build",
            source_relative: "tests/codegen/fail/unsupported_async_generic_main_build.ql",
            emit: "llvm-ir",
            expected_stderr_relative: "tests/codegen/fail/unsupported_async_generic_main_build.stderr",
            extra_args: &[],
        },
        FailCase {
            name: "unsupported_deferred_multi_segment_type_build",
            source_relative: "tests/codegen/fail/unsupported_deferred_multi_segment_type_build.ql",
            emit: "dylib",
            expected_stderr_relative: "tests/codegen/fail/unsupported_deferred_multi_segment_type_build.stderr",
            extra_args: &[],
        },
        FailCase {
            name: "dylib_requires_export_build",
            source_relative: "tests/codegen/fail/dylib_requires_export_build.ql",
            emit: "dylib",
            expected_stderr_relative: "tests/codegen/fail/dylib_requires_export_build.stderr",
            extra_args: &[],
        },
        FailCase {
            name: "executable_header_build",
            source_relative: "fixtures/codegen/pass/minimal_build.ql",
            emit: "exe",
            expected_stderr_relative: "tests/codegen/fail/executable_header_build.stderr",
            extra_args: &["--header"],
        },
    ];
    fail_cases.extend(dynamic_task_handle_fail_cases());

    let mut failures = Vec::new();

    for case in pass_cases {
        if let Err(message) = run_pass_case(&workspace_root, &case) {
            failures.push(message);
        }
    }

    for case in fail_cases {
        if let Err(message) = run_fail_case(&workspace_root, &case) {
            failures.push(message);
        }
    }

    assert!(
        failures.is_empty(),
        "codegen snapshot regressions:\n\n{}",
        failures.join("\n\n")
    );
}

#[test]
fn dynamic_task_handle_codegen_cases_match() {
    let workspace_root = workspace_root();
    let mut failures = Vec::new();

    for case in dynamic_task_handle_pass_cases() {
        if let Err(message) = run_pass_case(&workspace_root, &case) {
            failures.push(message);
        }
    }

    assert!(
        failures.is_empty(),
        "dynamic task-handle codegen regressions:\n\n{}",
        failures.join("\n\n")
    );
}

#[test]
fn dynamic_task_handle_fail_codegen_cases_match() {
    let workspace_root = workspace_root();
    let mut failures = Vec::new();

    for case in dynamic_task_handle_fail_cases() {
        if let Err(message) = run_fail_case(&workspace_root, &case) {
            failures.push(message);
        }
    }

    assert!(
        failures.is_empty(),
        "dynamic task-handle fail codegen regressions:\n\n{}",
        failures.join("\n\n")
    );
}

#[test]
fn projected_dynamic_task_handle_codegen_cases_match() {
    let workspace_root = workspace_root();
    let mut failures = Vec::new();

    for case in projected_dynamic_task_handle_pass_cases() {
        if let Err(message) = run_pass_case(&workspace_root, &case) {
            failures.push(message);
        }
    }

    assert!(
        failures.is_empty(),
        "projected dynamic task-handle codegen regressions:\n\n{}",
        failures.join("\n\n")
    );
}

#[test]
fn static_alias_backed_projected_root_dynamic_task_handle_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "async_program_main_aliased_projected_root_static_alias_backed_dynamic_task_handle_reinit_exe",
        source_relative: "fixtures/codegen/pass/async_program_main_aliased_projected_root_static_alias_backed_dynamic_task_handle_reinit.ql",
        emit: "exe",
        expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!(
            "static alias-backed projected-root dynamic task-handle codegen regression:\n\n{message}"
        );
    }
}

#[test]
fn guard_refined_static_alias_backed_projected_root_dynamic_task_handle_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "async_program_main_aliased_guard_refined_static_alias_backed_projected_root_dynamic_task_handle_reinit_exe",
        source_relative: "fixtures/codegen/pass/async_program_main_aliased_guard_refined_static_alias_backed_projected_root_dynamic_task_handle_reinit.ql",
        emit: "exe",
        expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!(
            "guard-refined static alias-backed projected-root dynamic task-handle codegen regression:\n\n{message}"
        );
    }
}

#[test]
fn guard_refined_static_alias_backed_projected_root_dynamic_task_handle_object_codegen_case_matches()
 {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "async_program_main_aliased_guard_refined_static_alias_backed_projected_root_dynamic_task_handle_reinit_object",
        source_relative: "fixtures/codegen/pass/async_program_main_aliased_guard_refined_static_alias_backed_projected_root_dynamic_task_handle_reinit.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!(
            "guard-refined static alias-backed projected-root dynamic task-handle object-codegen regression:\n\n{message}"
        );
    }
}

#[test]
fn guard_refined_static_alias_backed_projected_root_dynamic_task_handle_llvm_ir_codegen_case_matches()
 {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "async_program_main_aliased_guard_refined_static_alias_backed_projected_root_dynamic_task_handle_reinit_llvm_ir",
        source_relative: "fixtures/codegen/pass/async_program_main_aliased_guard_refined_static_alias_backed_projected_root_dynamic_task_handle_reinit.ql",
        emit: "llvm-ir",
        expected_relative: "tests/codegen/pass/async_program_main_aliased_guard_refined_static_alias_backed_projected_root_dynamic_task_handle_reinit.ll",
        mock_compiler: false,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!(
            "guard-refined static alias-backed projected-root dynamic task-handle llvm-ir regression:\n\n{message}"
        );
    }
}

#[test]
fn guarded_cleanup_dynamic_task_handle_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "guarded_cleanup_dynamic_task_handle_build",
        source_relative: "fixtures/codegen/pass/guarded_cleanup_dynamic_task_handle_build.ql",
        emit: "staticlib",
        expected_relative: "tests/codegen/pass/minimal_library.staticlib.txt",
        mock_compiler: true,
        mock_archiver: true,
        archiver_style: Some(current_archiver_style()),
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("guarded cleanup dynamic task-handle build regression:\n\n{message}");
    }
}

#[test]
fn direct_cleanup_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "direct_cleanup_build",
        source_relative: "fixtures/codegen/pass/cleanup_direct_call.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("direct cleanup build regression:\n\n{message}");
    }
}

#[test]
fn cleanup_block_assignment_expr_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "cleanup_block_assignment_expr_build",
        source_relative: "fixtures/codegen/pass/cleanup_block_assignment_expr.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("cleanup block assignment-expr build regression:\n\n{message}");
    }
}

#[test]
fn cleanup_value_assignment_expr_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "cleanup_value_assignment_expr_build",
        source_relative: "fixtures/codegen/pass/cleanup_value_assignment_expr.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("cleanup value assignment-expr build regression:\n\n{message}");
    }
}

#[test]
fn cleanup_if_value_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "cleanup_if_value_build",
        source_relative: "fixtures/codegen/pass/cleanup_if_value.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("cleanup if-value build regression:\n\n{message}");
    }
}

#[test]
fn cleanup_match_value_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "cleanup_match_value_build",
        source_relative: "fixtures/codegen/pass/cleanup_match_value.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("cleanup match-value build regression:\n\n{message}");
    }
}

#[test]
fn assignment_expr_value_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "assignment_expr_value_build",
        source_relative: "fixtures/codegen/pass/assignment_expr_value.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("assignment expr value build regression:\n\n{message}");
    }
}

#[test]
fn guard_assignment_expr_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "guard_assignment_expr_build",
        source_relative: "fixtures/codegen/pass/guard_assignment_expr.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("guard assignment expr build regression:\n\n{message}");
    }
}

#[test]
fn guard_assignment_call_arg_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "guard_assignment_call_arg_build",
        source_relative: "fixtures/codegen/pass/guard_assignment_call_arg.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("guard assignment call-arg build regression:\n\n{message}");
    }
}

#[test]
fn guard_if_value_call_arg_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "guard_if_value_call_arg_build",
        source_relative: "fixtures/codegen/pass/guard_if_value_call_arg.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("guard if-value call-arg build regression:\n\n{message}");
    }
}

#[test]
fn guard_match_value_call_arg_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "guard_match_value_call_arg_build",
        source_relative: "fixtures/codegen/pass/guard_match_value_call_arg.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("guard match-value call-arg build regression:\n\n{message}");
    }
}

#[test]
fn guard_match_callable_callee_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "guard_match_callable_callee_build",
        source_relative: "fixtures/codegen/pass/guard_match_callable_callee.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("guard match-callable callee build regression:\n\n{message}");
    }
}

#[test]
fn cleanup_await_value_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "cleanup_await_value_build",
        source_relative: "fixtures/codegen/pass/cleanup_await_value.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("cleanup await-value build regression:\n\n{message}");
    }
}

#[test]
fn cleanup_spawn_value_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "cleanup_spawn_value_build",
        source_relative: "fixtures/codegen/pass/cleanup_spawn_value.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("cleanup spawn-value build regression:\n\n{message}");
    }
}

#[test]
fn cleanup_await_guards_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "cleanup_await_guards_build",
        source_relative: "fixtures/codegen/pass/cleanup_await_guards.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("cleanup await-guard build regression:\n\n{message}");
    }
}

#[test]
fn cleanup_awaited_control_flow_roots_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "cleanup_awaited_control_flow_roots_build",
        source_relative: "fixtures/codegen/pass/cleanup_awaited_control_flow_roots.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("cleanup awaited control-flow root build regression:\n\n{message}");
    }
}

#[test]
fn cleanup_awaited_projection_async_callable_control_flow_scrutinees_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "cleanup_awaited_projection_async_callable_control_flow_scrutinees_build",
        source_relative: "fixtures/codegen/pass/cleanup_awaited_projection_async_callable_control_flow_scrutinees.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!(
            "cleanup awaited projection async callable control-flow scrutinee build regression:\n\n{message}"
        );
    }
}

#[test]
fn cleanup_awaited_helper_inline_guards_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "cleanup_awaited_helper_inline_guards_build",
        source_relative: "fixtures/codegen/pass/cleanup_awaited_helper_inline_guards.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("cleanup awaited helper/inline guard build regression:\n\n{message}");
    }
}

#[test]
fn cleanup_awaited_nested_runtime_projection_guards_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "cleanup_awaited_nested_runtime_projection_guards_build",
        source_relative: "fixtures/codegen/pass/cleanup_awaited_nested_runtime_projection_guards.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("cleanup awaited nested runtime projection guard build regression:\n\n{message}");
    }
}

#[test]
fn cleanup_awaited_helper_inline_scrutinees_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "cleanup_awaited_helper_inline_scrutinees_build",
        source_relative: "fixtures/codegen/pass/cleanup_awaited_helper_inline_scrutinees.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("cleanup awaited helper/inline scrutinee build regression:\n\n{message}");
    }
}

#[test]
fn cleanup_awaited_nested_runtime_projection_scrutinees_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "cleanup_awaited_nested_runtime_projection_scrutinees_build",
        source_relative: "fixtures/codegen/pass/cleanup_awaited_nested_runtime_projection_scrutinees.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!(
            "cleanup awaited nested runtime projection scrutinee build regression:\n\n{message}"
        );
    }
}

#[test]
fn cleanup_awaited_aggregate_binding_scrutinees_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "cleanup_awaited_aggregate_binding_scrutinees_build",
        source_relative: "fixtures/codegen/pass/cleanup_awaited_aggregate_binding_scrutinees.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("cleanup awaited aggregate binding scrutinee build regression:\n\n{message}");
    }
}

#[test]
fn cleanup_awaited_aggregate_destructuring_scrutinees_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "cleanup_awaited_aggregate_destructuring_scrutinees_build",
        source_relative: "fixtures/codegen/pass/cleanup_awaited_aggregate_destructuring_scrutinees.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("cleanup awaited aggregate destructuring scrutinee build regression:\n\n{message}");
    }
}

#[test]
fn awaited_aggregate_destructuring_scrutinees_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "awaited_aggregate_destructuring_scrutinees_build",
        source_relative: "fixtures/codegen/pass/awaited_aggregate_destructuring_scrutinees.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("awaited aggregate destructuring scrutinee build regression:\n\n{message}");
    }
}

#[test]
fn awaited_fixed_array_destructuring_scrutinees_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "awaited_fixed_array_destructuring_scrutinees_build",
        source_relative: "fixtures/codegen/pass/awaited_fixed_array_destructuring_scrutinees.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("awaited fixed-array destructuring scrutinee build regression:\n\n{message}");
    }
}

#[test]
fn fixed_array_bind_patterns_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "fixed_array_bind_patterns_build",
        source_relative: "fixtures/codegen/pass/fixed_array_bind_patterns.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("fixed-array bind-pattern build regression:\n\n{message}");
    }
}

#[test]
fn function_value_local_call_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "function_value_local_call_build",
        source_relative: "fixtures/codegen/pass/function_value_local_call.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("function value local call build regression:\n\n{message}");
    }
}

#[test]
fn capturing_closure_direct_local_call_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "capturing_closure_direct_local_call_build",
        source_relative: "fixtures/codegen/pass/capturing_closure_direct_local_call.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("capturing closure direct local call build regression:\n\n{message}");
    }
}

#[test]
fn capturing_closure_direct_local_string_call_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "capturing_closure_direct_local_string_call_build",
        source_relative: "fixtures/codegen/pass/capturing_closure_direct_local_string_call.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("capturing closure direct local string call build regression:\n\n{message}");
    }
}

#[test]
fn capturing_closure_task_handle_await_call_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "capturing_closure_task_handle_await_call_build",
        source_relative: "fixtures/codegen/pass/capturing_closure_task_handle_await_call.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("capturing closure task-handle await call build regression:\n\n{message}");
    }
}

#[test]
fn capturing_closure_task_handle_control_flow_await_call_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "capturing_closure_task_handle_control_flow_await_call_build",
        source_relative: "fixtures/codegen/pass/capturing_closure_task_handle_control_flow_await_call.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!(
            "capturing closure task-handle control-flow await call build regression:\n\n{message}"
        );
    }
}

#[test]
fn capturing_closure_task_handle_cleanup_await_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "capturing_closure_task_handle_cleanup_await_build",
        source_relative: "fixtures/codegen/pass/capturing_closure_task_handle_cleanup_await.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("capturing closure task-handle cleanup await build regression:\n\n{message}");
    }
}

#[test]
fn capturing_closure_task_handle_cleanup_await_root_matrix_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "capturing_closure_task_handle_cleanup_await_root_matrix_build",
        source_relative: "fixtures/codegen/pass/capturing_closure_task_handle_cleanup_await_root_matrix.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!(
            "capturing closure task-handle cleanup await root-matrix build regression:\n\n{message}"
        );
    }
}

#[test]
fn capturing_closure_task_handle_cleanup_await_alias_roots_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "capturing_closure_task_handle_cleanup_await_alias_roots_build",
        source_relative: "fixtures/codegen/pass/capturing_closure_task_handle_cleanup_await_alias_roots.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!(
            "capturing closure task-handle cleanup await alias-root build regression:\n\n{message}"
        );
    }
}

#[test]
fn capturing_closure_task_handle_cleanup_await_helper_inline_values_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "capturing_closure_task_handle_cleanup_await_helper_inline_values_build",
        source_relative: "fixtures/codegen/pass/capturing_closure_task_handle_cleanup_await_helper_inline_values.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!(
            "capturing closure task-handle cleanup await helper/inline values build regression:\n\n{message}"
        );
    }
}

#[test]
fn capturing_closure_task_handle_cleanup_await_nested_runtime_projection_values_codegen_case_matches()
 {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "capturing_closure_task_handle_cleanup_await_nested_runtime_projection_values_build",
        source_relative: "fixtures/codegen/pass/capturing_closure_task_handle_cleanup_await_nested_runtime_projection_values.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!(
            "capturing closure task-handle cleanup await nested runtime projection values build regression:\n\n{message}"
        );
    }
}

#[test]
fn capturing_closure_task_handle_cleanup_await_aggregate_binding_scrutinees_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "capturing_closure_task_handle_cleanup_await_aggregate_binding_scrutinees_build",
        source_relative: "fixtures/codegen/pass/capturing_closure_task_handle_cleanup_await_aggregate_binding_scrutinees.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!(
            "capturing closure task-handle cleanup await aggregate binding scrutinees build regression:\n\n{message}"
        );
    }
}

#[test]
fn capturing_closure_task_handle_cleanup_await_aggregate_destructuring_scrutinees_codegen_case_matches()
 {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "capturing_closure_task_handle_cleanup_await_aggregate_destructuring_scrutinees_build",
        source_relative: "fixtures/codegen/pass/capturing_closure_task_handle_cleanup_await_aggregate_destructuring_scrutinees.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!(
            "capturing closure task-handle cleanup await aggregate destructuring scrutinees build regression:\n\n{message}"
        );
    }
}

#[test]
fn capturing_closure_task_handle_cleanup_await_fixed_array_destructuring_scrutinees_codegen_case_matches()
 {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "capturing_closure_task_handle_cleanup_await_fixed_array_destructuring_scrutinees_build",
        source_relative: "fixtures/codegen/pass/capturing_closure_task_handle_cleanup_await_fixed_array_destructuring_scrutinees.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!(
            "capturing closure task-handle cleanup await fixed-array destructuring scrutinees build regression:\n\n{message}"
        );
    }
}

#[test]
fn capturing_closure_task_handle_cleanup_await_different_closure_roots_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "capturing_closure_task_handle_cleanup_await_different_closure_roots_build",
        source_relative: "fixtures/codegen/pass/capturing_closure_task_handle_cleanup_await_different_closure_roots.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!(
            "capturing closure task-handle cleanup await different-closure roots build regression:\n\n{message}"
        );
    }
}

#[test]
fn capturing_closure_task_handle_cleanup_await_different_closure_alias_roots_codegen_case_matches()
{
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "capturing_closure_task_handle_cleanup_await_different_closure_alias_roots_build",
        source_relative: "fixtures/codegen/pass/capturing_closure_task_handle_cleanup_await_different_closure_alias_roots.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!(
            "capturing closure task-handle cleanup await different-closure alias roots build regression:\n\n{message}"
        );
    }
}

#[test]
fn capturing_closure_task_handle_cleanup_await_shared_local_alias_chains_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "capturing_closure_task_handle_cleanup_await_shared_local_alias_chains_build",
        source_relative: "fixtures/codegen/pass/capturing_closure_task_handle_cleanup_await_shared_local_alias_chains.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!(
            "capturing closure task-handle cleanup await shared-local alias chains build regression:\n\n{message}"
        );
    }
}

#[test]
fn capturing_closure_task_handle_cleanup_await_guarded_match_shared_local_alias_chains_codegen_case_matches()
 {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "capturing_closure_task_handle_cleanup_await_guarded_match_shared_local_alias_chains_build",
        source_relative: "fixtures/codegen/pass/capturing_closure_task_handle_cleanup_await_guarded_match_shared_local_alias_chains.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!(
            "capturing closure task-handle cleanup await guarded-match shared-local alias chains build regression:\n\n{message}"
        );
    }
}

#[test]
fn capturing_closure_task_handle_cleanup_await_tagged_guarded_match_shared_local_alias_chains_codegen_case_matches()
 {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "capturing_closure_task_handle_cleanup_await_tagged_guarded_match_shared_local_alias_chains_build",
        source_relative: "fixtures/codegen/pass/capturing_closure_task_handle_cleanup_await_tagged_guarded_match_shared_local_alias_chains.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!(
            "capturing closure task-handle cleanup await tagged guarded-match shared-local alias chains build regression:\n\n{message}"
        );
    }
}

#[test]
fn capturing_closure_task_handle_cleanup_await_tagged_guarded_match_different_closure_roots_codegen_case_matches()
 {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "capturing_closure_task_handle_cleanup_await_tagged_guarded_match_different_closure_roots_build",
        source_relative: "fixtures/codegen/pass/capturing_closure_task_handle_cleanup_await_tagged_guarded_match_different_closure_roots.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!(
            "capturing closure task-handle cleanup await tagged guarded-match different-closure roots build regression:\n\n{message}"
        );
    }
}

#[test]
fn capturing_closure_task_handle_cleanup_await_tagged_guarded_match_different_closure_alias_roots_codegen_case_matches()
 {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "capturing_closure_task_handle_cleanup_await_tagged_guarded_match_different_closure_alias_roots_build",
        source_relative: "fixtures/codegen/pass/capturing_closure_task_handle_cleanup_await_tagged_guarded_match_different_closure_alias_roots.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!(
            "capturing closure task-handle cleanup await tagged guarded-match different-closure alias roots build regression:\n\n{message}"
        );
    }
}

#[test]
fn capturing_closure_task_handle_cleanup_await_guarded_match_different_closure_roots_codegen_case_matches()
 {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "capturing_closure_task_handle_cleanup_await_guarded_match_different_closure_roots_build",
        source_relative: "fixtures/codegen/pass/capturing_closure_task_handle_cleanup_await_guarded_match_different_closure_roots.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!(
            "capturing closure task-handle cleanup await guarded-match different-closure roots build regression:\n\n{message}"
        );
    }
}

#[test]
fn capturing_closure_task_handle_cleanup_await_guarded_match_different_closure_alias_roots_codegen_case_matches()
 {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "capturing_closure_task_handle_cleanup_await_guarded_match_different_closure_alias_roots_build",
        source_relative: "fixtures/codegen/pass/capturing_closure_task_handle_cleanup_await_guarded_match_different_closure_alias_roots.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!(
            "capturing closure task-handle cleanup await guarded-match different-closure alias roots build regression:\n\n{message}"
        );
    }
}

#[test]
fn capturing_closure_immutable_alias_call_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "capturing_closure_immutable_alias_call_build",
        source_relative: "fixtures/codegen/pass/capturing_closure_immutable_alias_call.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("capturing closure immutable alias call build regression:\n\n{message}");
    }
}

#[test]
fn capturing_closure_mutable_alias_reassign_call_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "capturing_closure_mutable_alias_reassign_call_build",
        source_relative: "fixtures/codegen/pass/capturing_closure_mutable_alias_reassign_call_build.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("capturing closure mutable alias reassign build regression:\n\n{message}");
    }
}

#[test]
fn capturing_closure_mutable_alias_cleanup_guard_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "capturing_closure_mutable_alias_cleanup_guard_build",
        source_relative: "fixtures/codegen/pass/capturing_closure_mutable_alias_cleanup_guard_build.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("capturing closure mutable alias cleanup/guard build regression:\n\n{message}");
    }
}

#[test]
fn different_target_mutable_capturing_closure_reassign_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "cleanup_block_different_target_mutable_capturing_closure_reassign_build",
        source_relative: "fixtures/codegen/pass/cleanup_block_different_target_mutable_capturing_closure_reassign_build.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!(
            "different-target mutable capturing-closure reassign build regression:\n\n{message}"
        );
    }
}

#[test]
fn capturing_closure_same_target_control_flow_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "capturing_closure_same_target_control_flow_build",
        source_relative: "fixtures/codegen/pass/capturing_closure_same_target_control_flow_build.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("capturing closure same-target control-flow build regression:\n\n{message}");
    }
}

#[test]
fn capturing_closure_ordinary_extended_call_roots_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "capturing_closure_ordinary_extended_call_roots_build",
        source_relative: "fixtures/codegen/pass/capturing_closure_ordinary_extended_call_roots_build.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("capturing closure ordinary extended call-root build regression:\n\n{message}");
    }
}

#[test]
fn callable_const_static_value_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "callable_const_static_value_build",
        source_relative: "fixtures/codegen/pass/callable_const_static_value.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("callable const/static value build regression:\n\n{message}");
    }
}

#[test]
fn cleanup_callable_const_alias_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "cleanup_callable_const_alias_build",
        source_relative: "fixtures/codegen/pass/cleanup_callable_const_alias.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("cleanup callable const alias build regression:\n\n{message}");
    }
}

#[test]
fn cleanup_foldable_function_item_calls_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "cleanup_foldable_function_item_calls_build",
        source_relative: "fixtures/codegen/pass/cleanup_foldable_function_item_calls.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("cleanup foldable function-item call build regression:\n\n{message}");
    }
}

#[test]
fn closure_backed_callable_cleanup_guard_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "closure_backed_callable_cleanup_guard_build",
        source_relative: "fixtures/codegen/pass/closure_backed_callable_cleanup_guard_build.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("closure-backed callable cleanup/guard build regression:\n\n{message}");
    }
}

#[test]
fn local_closure_cleanup_guard_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "local_closure_cleanup_guard_build",
        source_relative: "fixtures/codegen/pass/local_closure_cleanup_guard_build.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("local closure cleanup/guard build regression:\n\n{message}");
    }
}

#[test]
fn capturing_closure_cleanup_guard_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "capturing_closure_cleanup_guard_build",
        source_relative: "fixtures/codegen/pass/capturing_closure_cleanup_guard_build.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("capturing closure cleanup/guard build regression:\n\n{message}");
    }
}

#[test]
fn capturing_closure_cleanup_if_match_guard_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "capturing_closure_cleanup_if_match_guard_build",
        source_relative: "fixtures/codegen/pass/capturing_closure_cleanup_if_match_guard_build.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("capturing closure cleanup if/match guard build regression:\n\n{message}");
    }
}

#[test]
fn capturing_closure_cleanup_different_closure_call_roots_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "capturing_closure_cleanup_different_closure_call_roots_build",
        source_relative: "fixtures/codegen/pass/capturing_closure_cleanup_different_closure_call_roots_build.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!(
            "capturing closure cleanup different-closure call-root build regression:\n\n{message}"
        );
    }
}

#[test]
fn capturing_closure_ordinary_different_closure_call_roots_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "capturing_closure_ordinary_different_closure_call_roots_build",
        source_relative: "fixtures/codegen/pass/capturing_closure_ordinary_different_closure_call_roots_build.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!(
            "capturing closure ordinary different-closure call-root build regression:\n\n{message}"
        );
    }
}

#[test]
fn capturing_closure_ordinary_different_closure_binding_roots_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "capturing_closure_ordinary_different_closure_binding_roots_build",
        source_relative: "fixtures/codegen/pass/capturing_closure_ordinary_different_closure_binding_roots_build.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!(
            "capturing closure ordinary different-closure binding-root build regression:\n\n{message}"
        );
    }
}

#[test]
fn capturing_closure_ordinary_string_match_call_roots_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "capturing_closure_ordinary_string_match_call_roots_build",
        source_relative: "fixtures/codegen/pass/capturing_closure_ordinary_string_match_call_roots_build.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("capturing closure ordinary string-match call-root build regression:\n\n{message}");
    }
}

#[test]
fn capturing_closure_ordinary_guarded_string_match_call_roots_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "capturing_closure_ordinary_guarded_string_match_call_roots_build",
        source_relative: "fixtures/codegen/pass/capturing_closure_ordinary_guarded_string_match_call_roots_build.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!(
            "capturing closure ordinary guarded string-match call-root build regression:\n\n{message}"
        );
    }
}

#[test]
fn capturing_closure_match_guard_control_flow_local_alias_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "capturing_closure_match_guard_control_flow_local_alias_build",
        source_relative: "fixtures/codegen/pass/capturing_closure_match_guard_control_flow_local_alias_build.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!(
            "capturing closure match-guard control-flow local-alias build regression:\n\n{message}"
        );
    }
}

#[test]
fn capturing_closure_match_guard_bound_control_flow_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "capturing_closure_match_guard_bound_control_flow_build",
        source_relative: "fixtures/codegen/pass/capturing_closure_match_guard_bound_control_flow_build.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("capturing closure match-guard bound control-flow build regression:\n\n{message}");
    }
}

#[test]
fn capturing_closure_match_guard_block_assignment_bound_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "capturing_closure_match_guard_block_assignment_bound_build",
        source_relative: "fixtures/codegen/pass/capturing_closure_match_guard_block_assignment_bound_build.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!(
            "capturing closure match-guard block assignment-bound build regression:\n\n{message}"
        );
    }
}

#[test]
fn capturing_closure_match_guard_different_closure_block_alias_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "capturing_closure_match_guard_different_closure_block_alias_build",
        source_relative: "fixtures/codegen/pass/capturing_closure_match_guard_different_closure_block_alias_build.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!(
            "capturing closure match-guard different-closure block-alias build regression:\n\n{message}"
        );
    }
}

#[test]
fn capturing_closure_match_guard_different_closure_block_binding_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "capturing_closure_match_guard_different_closure_block_binding_build",
        source_relative: "fixtures/codegen/pass/capturing_closure_match_guard_different_closure_block_binding_build.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!(
            "capturing closure match-guard different-closure block-binding build regression:\n\n{message}"
        );
    }
}

#[test]
fn match_guard_callable_alias_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "match_guard_callable_alias_build",
        source_relative: "fixtures/codegen/pass/match_guard_callable_alias.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("match guard callable alias build regression:\n\n{message}");
    }
}

#[test]
fn cleanup_match_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "cleanup_match_build",
        source_relative: "fixtures/codegen/pass/cleanup_match_call.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("cleanup match build regression:\n\n{message}");
    }
}

#[test]
fn cleanup_match_callable_guard_alias_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "cleanup_match_callable_guard_alias_build",
        source_relative: "fixtures/codegen/pass/cleanup_match_callable_guard_alias.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("cleanup match callable guard alias build regression:\n\n{message}");
    }
}

#[test]
fn cleanup_string_match_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "cleanup_string_match_build",
        source_relative: "fixtures/codegen/pass/cleanup_string_match_build.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("cleanup string match build regression:\n\n{message}");
    }
}

#[test]
fn cleanup_match_binding_arm_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "cleanup_match_binding_arm_build",
        source_relative: "fixtures/codegen/pass/cleanup_match_binding_arm.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("cleanup match binding arm build regression:\n\n{message}");
    }
}

#[test]
fn cleanup_branch_async_blocks_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "cleanup_branch_async_blocks_build",
        source_relative: "fixtures/codegen/pass/cleanup_branch_async_blocks.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("cleanup async branch-block build regression:\n\n{message}");
    }
}

#[test]
fn callable_callee_control_flow_roots_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "callable_callee_control_flow_roots_build",
        source_relative: "fixtures/codegen/pass/callable_callee_control_flow_roots.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("callable callee control-flow root build regression:\n\n{message}");
    }
}

#[test]
fn awaited_guard_async_callable_control_flow_roots_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "awaited_guard_async_callable_control_flow_roots_build",
        source_relative: "fixtures/codegen/pass/awaited_guard_async_callable_control_flow_roots.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("awaited guard async callable control-flow root build regression:\n\n{message}");
    }
}

#[test]
fn awaited_scrutinee_async_callable_control_flow_roots_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "awaited_scrutinee_async_callable_control_flow_roots_build",
        source_relative: "fixtures/codegen/pass/awaited_scrutinee_async_callable_control_flow_roots.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("awaited scrutinee async callable control-flow root build regression:\n\n{message}");
    }
}

#[test]
fn awaited_projection_async_callable_control_flow_scrutinees_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "awaited_projection_async_callable_control_flow_scrutinees_build",
        source_relative: "fixtures/codegen/pass/awaited_projection_async_callable_control_flow_scrutinees.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!(
            "awaited projection async callable control-flow scrutinee build regression:\n\n{message}"
        );
    }
}

#[test]
fn awaited_aggregate_binding_scrutinees_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "awaited_aggregate_binding_scrutinees_build",
        source_relative: "fixtures/codegen/pass/awaited_aggregate_binding_scrutinees.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("awaited aggregate binding scrutinee build regression:\n\n{message}");
    }
}

#[test]
fn awaited_projection_async_callable_control_flow_guard_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "awaited_projection_async_callable_control_flow_guard_build",
        source_relative: "fixtures/codegen/pass/awaited_projection_async_callable_control_flow_guard.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!(
            "awaited projection async callable control-flow guard build regression:\n\n{message}"
        );
    }
}

#[test]
fn awaited_aggregate_guard_async_callable_control_flow_roots_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "awaited_aggregate_guard_async_callable_control_flow_roots_build",
        source_relative: "fixtures/codegen/pass/awaited_aggregate_guard_async_callable_control_flow_roots.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!(
            "awaited aggregate guard async callable control-flow root build regression:\n\n{message}"
        );
    }
}

#[test]
fn awaited_call_backed_aggregate_guard_async_callable_control_flow_roots_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "awaited_call_backed_aggregate_guard_async_callable_control_flow_roots_build",
        source_relative: "fixtures/codegen/pass/awaited_call_backed_aggregate_guard_async_callable_control_flow_roots.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!(
            "awaited call-backed aggregate guard async callable control-flow root build regression:\n\n{message}"
        );
    }
}

#[test]
fn awaited_guard_import_alias_helpers_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "awaited_guard_import_alias_helpers_build",
        source_relative: "fixtures/codegen/pass/awaited_guard_import_alias_helpers.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("awaited guard import-alias helper build regression:\n\n{message}");
    }
}

#[test]
fn awaited_nested_call_root_runtime_projection_guards_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "awaited_nested_call_root_runtime_projection_guards_build",
        source_relative: "fixtures/codegen/pass/awaited_nested_call_root_runtime_projection_guards.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("awaited nested call-root runtime projection guard build regression:\n\n{message}");
    }
}

#[test]
fn awaited_inline_guard_families_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "awaited_inline_guard_families_build",
        source_relative: "fixtures/codegen/pass/awaited_inline_guard_families.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("awaited inline guard family build regression:\n\n{message}");
    }
}

#[test]
fn awaited_scrutinee_families_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "awaited_scrutinee_families_build",
        source_relative: "fixtures/codegen/pass/awaited_scrutinee_families.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("awaited scrutinee family build regression:\n\n{message}");
    }
}

#[test]
fn cleanup_block_sequence_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "cleanup_block_sequence_build",
        source_relative: "fixtures/codegen/pass/cleanup_block_sequence.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("cleanup block sequence build regression:\n\n{message}");
    }
}

#[test]
fn cleanup_block_let_binding_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "cleanup_block_let_binding_build",
        source_relative: "fixtures/codegen/pass/cleanup_block_let_binding.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("cleanup block let binding build regression:\n\n{message}");
    }
}

#[test]
fn cleanup_block_capturing_closure_alias_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "cleanup_block_capturing_closure_alias_build",
        source_relative: "fixtures/codegen/pass/cleanup_block_capturing_closure_alias_build.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("cleanup block capturing-closure alias build regression:\n\n{message}");
    }
}

#[test]
fn cleanup_block_mutable_capturing_closure_reassign_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "cleanup_block_mutable_capturing_closure_reassign_build",
        source_relative: "fixtures/codegen/pass/cleanup_block_mutable_capturing_closure_reassign_build.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("cleanup block mutable capturing-closure reassign build regression:\n\n{message}");
    }
}

#[test]
fn cleanup_assignment_valued_capturing_closure_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "cleanup_assignment_valued_capturing_closure_build",
        source_relative: "fixtures/codegen/pass/cleanup_assignment_valued_capturing_closure_build.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("cleanup assignment-valued capturing-closure build regression:\n\n{message}");
    }
}

#[test]
fn cleanup_control_flow_assignment_valued_capturing_closure_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "cleanup_control_flow_assignment_valued_capturing_closure_build",
        source_relative: "fixtures/codegen/pass/cleanup_control_flow_assignment_valued_capturing_closure_build.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!(
            "cleanup control-flow assignment-valued capturing-closure build regression:\n\n{message}"
        );
    }
}

#[test]
fn cleanup_if_shared_local_control_flow_capturing_closure_alias_chain_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "cleanup_if_shared_local_control_flow_capturing_closure_alias_chain_build",
        source_relative: "fixtures/codegen/pass/cleanup_if_shared_local_control_flow_capturing_closure_alias_chain_build.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!(
            "cleanup if shared-local control-flow capturing-closure alias-chain build regression:\n\n{message}"
        );
    }
}

#[test]
fn cleanup_match_shared_local_control_flow_capturing_closure_alias_chain_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "cleanup_match_shared_local_control_flow_capturing_closure_alias_chain_build",
        source_relative: "fixtures/codegen/pass/cleanup_match_shared_local_control_flow_capturing_closure_alias_chain_build.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!(
            "cleanup match shared-local control-flow capturing-closure alias-chain build regression:\n\n{message}"
        );
    }
}

#[test]
fn cleanup_guarded_match_shared_local_control_flow_capturing_closure_alias_chain_codegen_case_matches()
 {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "cleanup_guarded_match_shared_local_control_flow_capturing_closure_alias_chain_build",
        source_relative: "fixtures/codegen/pass/cleanup_guarded_match_shared_local_control_flow_capturing_closure_alias_chain_build.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!(
            "cleanup guarded match shared-local control-flow capturing-closure alias-chain build regression:\n\n{message}"
        );
    }
}

#[test]
fn cleanup_block_assignment_valued_capturing_closure_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "cleanup_block_assignment_valued_capturing_closure_build",
        source_relative: "fixtures/codegen/pass/cleanup_block_assignment_valued_capturing_closure_build.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("cleanup block assignment-valued capturing-closure build regression:\n\n{message}");
    }
}

#[test]
fn cleanup_block_control_flow_assignment_valued_capturing_closure_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "cleanup_block_control_flow_assignment_valued_capturing_closure_build",
        source_relative: "fixtures/codegen/pass/cleanup_block_control_flow_assignment_valued_capturing_closure_build.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!(
            "cleanup block control-flow assignment-valued capturing-closure build regression:\n\n{message}"
        );
    }
}

#[test]
fn cleanup_block_control_flow_local_alias_capturing_closure_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "cleanup_block_control_flow_local_alias_capturing_closure_build",
        source_relative: "fixtures/codegen/pass/cleanup_block_control_flow_local_alias_capturing_closure_build.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!(
            "cleanup block control-flow local-alias capturing-closure build regression:\n\n{message}"
        );
    }
}

#[test]
fn cleanup_control_flow_local_alias_capturing_closure_call_roots_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "cleanup_control_flow_local_alias_capturing_closure_call_roots_build",
        source_relative: "fixtures/codegen/pass/cleanup_control_flow_local_alias_capturing_closure_call_roots_build.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!(
            "cleanup control-flow local-alias capturing-closure call-root build regression:\n\n{message}"
        );
    }
}

#[test]
fn cleanup_block_let_destructuring_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "cleanup_block_let_destructuring_build",
        source_relative: "fixtures/codegen/pass/cleanup_block_let_destructuring.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("cleanup block let destructuring build regression:\n\n{message}");
    }
}

#[test]
fn cleanup_fixed_array_destructuring_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "cleanup_fixed_array_destructuring_build",
        source_relative: "fixtures/codegen/pass/cleanup_fixed_array_destructuring.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("cleanup fixed-array destructuring build regression:\n\n{message}");
    }
}

#[test]
fn fixed_array_match_catch_all_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "fixed_array_match_catch_all_build",
        source_relative: "fixtures/codegen/pass/fixed_array_match_catch_all.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("fixed-array match catch-all build regression:\n\n{message}");
    }
}

#[test]
fn tuple_struct_match_catch_all_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "tuple_struct_match_catch_all_build",
        source_relative: "fixtures/codegen/pass/tuple_struct_match_catch_all.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("tuple/struct match catch-all build regression:\n\n{message}");
    }
}

#[test]
fn projected_aggregate_match_catch_all_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "projected_aggregate_match_catch_all_build",
        source_relative: "fixtures/codegen/pass/projected_aggregate_match_catch_all.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("projected aggregate match catch-all build regression:\n\n{message}");
    }
}

#[test]
fn import_alias_projected_aggregate_match_catch_all_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "import_alias_projected_aggregate_match_catch_all_build",
        source_relative: "fixtures/codegen/pass/import_alias_projected_aggregate_match_catch_all.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("import-alias projected aggregate match catch-all build regression:\n\n{message}");
    }
}

#[test]
fn import_alias_control_flow_projected_aggregate_match_catch_all_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "import_alias_control_flow_projected_aggregate_match_catch_all_build",
        source_relative: "fixtures/codegen/pass/import_alias_control_flow_projected_aggregate_match_catch_all.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!(
            "import-alias control-flow projected aggregate match catch-all build regression:\n\n{message}"
        );
    }
}

#[test]
fn control_flow_projected_aggregate_match_catch_all_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "control_flow_projected_aggregate_match_catch_all_build",
        source_relative: "fixtures/codegen/pass/control_flow_projected_aggregate_match_catch_all.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("control-flow projected aggregate match catch-all build regression:\n\n{message}");
    }
}

#[test]
fn nested_projected_aggregate_match_catch_all_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "nested_projected_aggregate_match_catch_all_build",
        source_relative: "fixtures/codegen/pass/nested_projected_aggregate_match_catch_all.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("nested projected aggregate match catch-all build regression:\n\n{message}");
    }
}

#[test]
fn import_alias_nested_projected_aggregate_match_catch_all_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "import_alias_nested_projected_aggregate_match_catch_all_build",
        source_relative: "fixtures/codegen/pass/import_alias_nested_projected_aggregate_match_catch_all.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("import-alias nested projected aggregate match catch-all build regression:\n\n{message}");
    }
}

#[test]
fn control_flow_nested_projected_aggregate_match_catch_all_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "control_flow_nested_projected_aggregate_match_catch_all_build",
        source_relative: "fixtures/codegen/pass/control_flow_nested_projected_aggregate_match_catch_all.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("control-flow nested projected aggregate match catch-all build regression:\n\n{message}");
    }
}

#[test]
fn import_alias_control_flow_nested_projected_aggregate_match_catch_all_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "import_alias_control_flow_nested_projected_aggregate_match_catch_all_build",
        source_relative:
            "fixtures/codegen/pass/import_alias_control_flow_nested_projected_aggregate_match_catch_all.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!(
            "import-alias control-flow nested projected aggregate match catch-all build regression:\n\n{message}"
        );
    }
}

#[test]
fn call_root_aggregate_match_catch_all_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "call_root_aggregate_match_catch_all_build",
        source_relative: "fixtures/codegen/pass/call_root_aggregate_match_catch_all.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("call-root aggregate match catch-all build regression:\n\n{message}");
    }
}

#[test]
fn import_alias_call_root_aggregate_match_catch_all_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "import_alias_call_root_aggregate_match_catch_all_build",
        source_relative: "fixtures/codegen/pass/import_alias_call_root_aggregate_match_catch_all.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("import-alias call-root aggregate match catch-all build regression:\n\n{message}");
    }
}

#[test]
fn nested_call_root_aggregate_match_catch_all_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "nested_call_root_aggregate_match_catch_all_build",
        source_relative: "fixtures/codegen/pass/nested_call_root_aggregate_match_catch_all.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("nested call-root aggregate match catch-all build regression:\n\n{message}");
    }
}

#[test]
fn import_alias_nested_call_root_aggregate_match_catch_all_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "import_alias_nested_call_root_aggregate_match_catch_all_build",
        source_relative: "fixtures/codegen/pass/import_alias_nested_call_root_aggregate_match_catch_all.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!(
            "import-alias nested call-root aggregate match catch-all build regression:\n\n{message}"
        );
    }
}

#[test]
fn control_flow_nested_call_root_aggregate_match_catch_all_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "control_flow_nested_call_root_aggregate_match_catch_all_build",
        source_relative: "fixtures/codegen/pass/control_flow_nested_call_root_aggregate_match_catch_all.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!(
            "control-flow nested call-root aggregate match catch-all build regression:\n\n{message}"
        );
    }
}

#[test]
fn import_alias_control_flow_nested_call_root_aggregate_match_catch_all_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "import_alias_control_flow_nested_call_root_aggregate_match_catch_all_build",
        source_relative: "fixtures/codegen/pass/import_alias_control_flow_nested_call_root_aggregate_match_catch_all.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!(
            "import-alias control-flow nested call-root aggregate match catch-all build regression:\n\n{message}"
        );
    }
}

#[test]
fn awaited_projected_aggregate_match_catch_all_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "awaited_projected_aggregate_match_catch_all_build",
        source_relative: "fixtures/codegen/pass/awaited_projected_aggregate_match_catch_all.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("awaited projected aggregate match catch-all build regression:\n\n{message}");
    }
}

#[test]
fn import_alias_awaited_projected_aggregate_match_catch_all_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "import_alias_awaited_projected_aggregate_match_catch_all_build",
        source_relative: "fixtures/codegen/pass/import_alias_awaited_projected_aggregate_match_catch_all.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!(
            "import-alias awaited projected aggregate match catch-all build regression:\n\n{message}"
        );
    }
}

#[test]
fn control_flow_awaited_projected_aggregate_match_catch_all_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "control_flow_awaited_projected_aggregate_match_catch_all_build",
        source_relative: "fixtures/codegen/pass/control_flow_awaited_projected_aggregate_match_catch_all.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!(
            "control-flow awaited projected aggregate match catch-all build regression:\n\n{message}"
        );
    }
}

#[test]
fn import_alias_control_flow_awaited_projected_aggregate_match_catch_all_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "import_alias_control_flow_awaited_projected_aggregate_match_catch_all_build",
        source_relative: "fixtures/codegen/pass/import_alias_control_flow_awaited_projected_aggregate_match_catch_all.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!(
            "import-alias control-flow awaited projected aggregate match catch-all build regression:\n\n{message}"
        );
    }
}

#[test]
fn awaited_nested_projected_aggregate_match_catch_all_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "awaited_nested_projected_aggregate_match_catch_all_build",
        source_relative: "fixtures/codegen/pass/awaited_nested_projected_aggregate_match_catch_all.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("awaited nested projected aggregate match catch-all build regression:\n\n{message}");
    }
}

#[test]
fn import_alias_awaited_nested_projected_aggregate_match_catch_all_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "import_alias_awaited_nested_projected_aggregate_match_catch_all_build",
        source_relative: "fixtures/codegen/pass/import_alias_awaited_nested_projected_aggregate_match_catch_all.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!(
            "import-alias awaited nested projected aggregate match catch-all build regression:\n\n{message}"
        );
    }
}

#[test]
fn control_flow_awaited_nested_projected_aggregate_match_catch_all_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "control_flow_awaited_nested_projected_aggregate_match_catch_all_build",
        source_relative: "fixtures/codegen/pass/control_flow_awaited_nested_projected_aggregate_match_catch_all.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!(
            "control-flow awaited nested projected aggregate match catch-all build regression:\n\n{message}"
        );
    }
}

#[test]
fn import_alias_control_flow_awaited_nested_projected_aggregate_match_catch_all_codegen_case_matches()
 {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "import_alias_control_flow_awaited_nested_projected_aggregate_match_catch_all_build",
        source_relative: "fixtures/codegen/pass/import_alias_control_flow_awaited_nested_projected_aggregate_match_catch_all.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!(
            "import-alias control-flow awaited nested projected aggregate match catch-all build regression:

{message}"
        );
    }
}

#[test]
fn cleanup_block_while_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "cleanup_block_while_build",
        source_relative: "fixtures/codegen/pass/cleanup_block_while.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("cleanup block while build regression:\n\n{message}");
    }
}

#[test]
fn cleanup_block_while_break_continue_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "cleanup_block_while_break_continue_build",
        source_relative: "fixtures/codegen/pass/cleanup_block_while_break_continue.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("cleanup block while break/continue build regression:\n\n{message}");
    }
}

#[test]
fn cleanup_block_loop_break_continue_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "cleanup_block_loop_break_continue_build",
        source_relative: "fixtures/codegen/pass/cleanup_block_loop_break_continue.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("cleanup block loop break/continue build regression:\n\n{message}");
    }
}

#[test]
fn cleanup_block_for_fixed_shapes_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "cleanup_block_for_fixed_shapes_build",
        source_relative: "fixtures/codegen/pass/cleanup_block_for_fixed_shapes.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("cleanup block for fixed-shape build regression:\n\n{message}");
    }
}

#[test]
fn cleanup_block_for_await_fixed_shapes_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "cleanup_block_for_await_fixed_shapes_build",
        source_relative: "fixtures/codegen/pass/cleanup_block_for_await_fixed_shapes.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("cleanup block for-await fixed-shape build regression:\n\n{message}");
    }
}

#[test]
fn cleanup_block_for_await_call_roots_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "cleanup_block_for_await_call_roots_build",
        source_relative: "fixtures/codegen/pass/cleanup_block_for_await_call_roots.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("cleanup block for-await call-root build regression:\n\n{message}");
    }
}

#[test]
fn cleanup_block_for_await_direct_control_flow_roots_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "cleanup_block_for_await_direct_control_flow_roots_build",
        source_relative: "fixtures/codegen/pass/cleanup_block_for_await_direct_control_flow_roots.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("cleanup block for-await direct control-flow roots build regression:\n\n{message}");
    }
}

#[test]
fn cleanup_block_for_await_direct_question_roots_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "cleanup_block_for_await_direct_question_roots_build",
        source_relative: "fixtures/codegen/pass/cleanup_block_for_await_direct_question_roots.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("cleanup block for-await direct question-root build regression:\n\n{message}");
    }
}

#[test]
fn cleanup_block_for_await_scalar_item_roots_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "cleanup_block_for_await_scalar_item_roots_build",
        source_relative: "fixtures/codegen/pass/cleanup_block_for_await_scalar_item_roots.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("cleanup block for-await scalar item-root build regression:\n\n{message}");
    }
}

#[test]
fn task_item_roots_for_await_and_cleanup_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "task_item_roots_for_await_and_cleanup_build",
        source_relative: "fixtures/codegen/pass/task_item_roots_for_await_and_cleanup.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("task item-root for-await build regression:\n\n{message}");
    }
}

#[test]
fn projected_task_item_roots_for_await_and_cleanup_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "projected_task_item_roots_for_await_and_cleanup_build",
        source_relative: "fixtures/codegen/pass/projected_task_item_roots_for_await_and_cleanup.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("projected task item-root for-await build regression:\n\n{message}");
    }
}

#[test]
fn task_item_value_flow_in_async_builds_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "task_item_value_flow_in_async_builds_build",
        source_relative: "fixtures/codegen/pass/task_item_value_flow_in_async_builds.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("task item-value async flow build regression:\n\n{message}");
    }
}

#[test]
fn callable_value_control_flow_in_async_and_cleanup_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "callable_value_control_flow_in_async_and_cleanup_build",
        source_relative: "fixtures/codegen/pass/callable_value_control_flow_in_async_and_cleanup.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("callable value control-flow async/cleanup build regression:\n\n{message}");
    }
}

#[test]
fn cleanup_block_for_await_inline_task_roots_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "cleanup_block_for_await_inline_task_roots_build",
        source_relative: "fixtures/codegen/pass/cleanup_block_for_await_inline_task_roots.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("cleanup block for-await inline-task roots build regression:\n\n{message}");
    }
}

#[test]
fn cleanup_block_for_await_awaited_projected_root_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "cleanup_block_for_await_awaited_projected_root_build",
        source_relative: "fixtures/codegen/pass/cleanup_block_for_await_awaited_projected_root.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("cleanup block for-await awaited-projected root build regression:\n\n{message}");
    }
}

#[test]
fn cleanup_block_let_struct_literal_with_awaited_projected_field_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "cleanup_block_let_struct_literal_with_awaited_projected_field_build",
        source_relative: "fixtures/codegen/pass/cleanup_block_let_struct_literal_with_awaited_projected_field.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("cleanup block let-struct awaited-projected field build regression:\n\n{message}");
    }
}

#[test]
fn cleanup_block_for_await_projected_if_match_roots_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "cleanup_block_for_await_projected_if_match_roots_build",
        source_relative: "fixtures/codegen/pass/cleanup_block_for_await_projected_if_match_roots.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("cleanup block for-await projected if/match roots build regression:\n\n{message}");
    }
}

#[test]
fn cleanup_block_for_await_projected_block_root_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "cleanup_block_for_await_projected_block_root_build",
        source_relative: "fixtures/codegen/pass/cleanup_block_for_await_projected_block_root.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("cleanup block for-await projected block-root build regression:\n\n{message}");
    }
}

#[test]
fn cleanup_block_for_await_projected_assignment_root_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "cleanup_block_for_await_projected_assignment_root_build",
        source_relative: "fixtures/codegen/pass/cleanup_block_for_await_projected_assignment_root.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("cleanup block for-await projected assignment-root build regression:\n\n{message}");
    }
}

#[test]
fn cleanup_block_for_await_projected_question_root_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "cleanup_block_for_await_projected_question_root_build",
        source_relative: "fixtures/codegen/pass/cleanup_block_for_await_projected_question_root.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("cleanup block for-await projected question-root build regression:\n\n{message}");
    }
}

#[test]
fn fixed_shape_for_projected_control_flow_roots_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "fixed_shape_for_projected_control_flow_roots_build",
        source_relative: "fixtures/codegen/pass/fixed_shape_for_projected_control_flow_roots.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("fixed-shape for projected control-flow roots build regression:\n\n{message}");
    }
}

#[test]
fn fixed_shape_for_await_projected_control_flow_roots_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "fixed_shape_for_await_projected_control_flow_roots_build",
        source_relative: "fixtures/codegen/pass/fixed_shape_for_await_projected_control_flow_roots.ql",
        emit: "staticlib",
        expected_relative: "tests/codegen/pass/minimal_library.staticlib.txt",
        mock_compiler: true,
        mock_archiver: true,
        archiver_style: Some(current_archiver_style()),
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("fixed-shape for-await projected control-flow roots build regression:\n\n{message}");
    }
}

#[test]
fn cleanup_block_for_projected_question_root_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "cleanup_block_for_projected_question_root_build",
        source_relative: "fixtures/codegen/pass/cleanup_block_for_projected_question_root.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("cleanup block for projected question-root build regression:\n\n{message}");
    }
}

#[test]
fn cleanup_block_for_destructuring_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "cleanup_block_for_destructuring_build",
        source_relative: "fixtures/codegen/pass/cleanup_block_for_destructuring.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("cleanup block destructuring for build regression:\n\n{message}");
    }
}

#[test]
fn cleanup_block_for_projected_call_roots_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "cleanup_block_for_projected_call_roots_build",
        source_relative: "fixtures/codegen/pass/cleanup_block_for_projected_call_roots.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("cleanup block projected/call-root for build regression:\n\n{message}");
    }
}

#[test]
fn cleanup_block_for_alias_nested_call_roots_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "cleanup_block_for_alias_nested_call_roots_build",
        source_relative: "fixtures/codegen/pass/cleanup_block_for_alias_nested_call_roots.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("cleanup block alias/nested-call-root for build regression:\n\n{message}");
    }
}

#[test]
fn cleanup_block_for_const_static_roots_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "cleanup_block_for_const_static_roots_build",
        source_relative: "fixtures/codegen/pass/cleanup_block_for_const_static_roots.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("cleanup block const/static-root for build regression:\n\n{message}");
    }
}

#[test]
fn bind_pattern_destructuring_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "bind_pattern_destructuring_build",
        source_relative: "fixtures/codegen/pass/bind_pattern_destructuring.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("bind-pattern destructuring build regression:\n\n{message}");
    }
}

#[test]
fn cleanup_block_guard_scrutinee_value_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "cleanup_block_guard_scrutinee_value_build",
        source_relative: "fixtures/codegen/pass/cleanup_block_guard_scrutinee_value.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("cleanup block guard/scrutinee/value build regression:\n\n{message}");
    }
}

#[test]
fn cleanup_foldable_control_flow_values_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "cleanup_foldable_control_flow_values_build",
        source_relative: "fixtures/codegen/pass/cleanup_foldable_control_flow_values.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("cleanup foldable control-flow values build regression:\n\n{message}");
    }
}

#[test]
fn match_question_mark_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "match_question_mark_build",
        source_relative: "fixtures/codegen/pass/match_question_mark.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("match question-mark build regression:\n\n{message}");
    }
}

#[test]
fn cleanup_question_mark_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "cleanup_question_mark_build",
        source_relative: "fixtures/codegen/pass/cleanup_question_mark.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("cleanup question-mark build regression:\n\n{message}");
    }
}

#[test]
fn cleanup_internal_question_mark_codegen_case_matches() {
    let workspace_root = workspace_root();
    let case = PassCase {
        name: "cleanup_internal_question_mark_build",
        source_relative: "fixtures/codegen/pass/cleanup_internal_question_mark.ql",
        emit: "obj",
        expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
        mock_compiler: true,
        mock_archiver: false,
        archiver_style: None,
        header_surface: None,
        expected_header_relative: None,
    };

    if let Err(message) = run_pass_case(&workspace_root, &case) {
        panic!("cleanup internal question-mark build regression:\n\n{message}");
    }
}

#[derive(Clone, Copy)]
struct PassCase {
    name: &'static str,
    source_relative: &'static str,
    emit: &'static str,
    expected_relative: &'static str,
    mock_compiler: bool,
    mock_archiver: bool,
    archiver_style: Option<&'static str>,
    header_surface: Option<&'static str>,
    expected_header_relative: Option<&'static str>,
}

#[derive(Clone, Copy)]
struct FailCase {
    name: &'static str,
    source_relative: &'static str,
    emit: &'static str,
    expected_stderr_relative: &'static str,
    extra_args: &'static [&'static str],
}

fn projected_dynamic_task_handle_pass_cases() -> Vec<PassCase> {
    vec![
        PassCase {
            name: "async_program_main_projected_dynamic_task_handle_reinit_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_projected_dynamic_task_handle_reinit.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_projected_dynamic_task_handle_conditional_reinit_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_projected_dynamic_task_handle_conditional_reinit.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_guard_refined_dynamic_task_handle_reinit_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_guard_refined_dynamic_task_handle_reinit.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_guard_refined_projected_dynamic_task_handle_reinit_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_guard_refined_projected_dynamic_task_handle_reinit.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_projected_root_dynamic_task_handle_reinit_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_projected_root_dynamic_task_handle_reinit.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_projected_root_const_backed_dynamic_task_handle_reinit_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_projected_root_const_backed_dynamic_task_handle_reinit.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_aliased_projected_root_dynamic_task_handle_reinit_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_aliased_projected_root_dynamic_task_handle_reinit.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_aliased_projected_root_const_backed_dynamic_task_handle_reinit_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_aliased_projected_root_const_backed_dynamic_task_handle_reinit.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_aliased_projected_root_static_alias_backed_dynamic_task_handle_reinit_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_aliased_projected_root_static_alias_backed_dynamic_task_handle_reinit.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_aliased_guard_refined_projected_root_dynamic_task_handle_reinit_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_aliased_guard_refined_projected_root_dynamic_task_handle_reinit.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_aliased_guard_refined_const_backed_projected_root_dynamic_task_handle_reinit_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_aliased_guard_refined_const_backed_projected_root_dynamic_task_handle_reinit.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_aliased_guard_refined_static_alias_backed_projected_root_dynamic_task_handle_reinit_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_aliased_guard_refined_static_alias_backed_projected_root_dynamic_task_handle_reinit.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_aliased_projected_root_task_handle_tuple_repackage_reinit_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_aliased_projected_root_task_handle_tuple_repackage_reinit.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_aliased_projected_root_task_handle_struct_repackage_reinit_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_aliased_projected_root_task_handle_struct_repackage_reinit.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_aliased_projected_root_task_handle_nested_repackage_reinit_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_aliased_projected_root_task_handle_nested_repackage_reinit.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_aliased_projected_root_task_handle_nested_repackage_spawn_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_aliased_projected_root_task_handle_nested_repackage_spawn.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_aliased_projected_root_task_handle_array_repackage_spawn_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_aliased_projected_root_task_handle_array_repackage_spawn.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_aliased_projected_root_task_handle_nested_array_repackage_spawn_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_aliased_projected_root_task_handle_nested_array_repackage_spawn.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_aliased_guard_refined_const_backed_projected_root_task_handle_nested_array_repackage_spawn_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_aliased_guard_refined_const_backed_projected_root_task_handle_nested_array_repackage_spawn.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_aliased_projected_root_task_handle_forwarded_nested_array_repackage_spawn_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_aliased_projected_root_task_handle_forwarded_nested_array_repackage_spawn.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_aliased_guard_refined_const_backed_projected_root_task_handle_forwarded_nested_array_repackage_spawn_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_aliased_guard_refined_const_backed_projected_root_task_handle_forwarded_nested_array_repackage_spawn.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_composed_dynamic_task_handle_nested_array_repackage_spawn_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_composed_dynamic_task_handle_nested_array_repackage_spawn.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_alias_sourced_composed_dynamic_task_handle_nested_array_repackage_spawn_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_alias_sourced_composed_dynamic_task_handle_nested_array_repackage_spawn.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_guarded_alias_sourced_composed_dynamic_task_handle_nested_array_repackage_spawn_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_alias_sourced_composed_dynamic_task_handle_nested_array_repackage_spawn.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn_with_tail_field_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn_with_tail_field.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_guarded_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn_with_tail_field_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn_with_tail_field.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_const_backed_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_const_backed_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_guarded_const_backed_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_guarded_const_backed_double_root_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_double_root_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_guarded_const_backed_double_root_double_source_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_double_root_double_source_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_guarded_const_backed_double_root_double_source_row_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_double_root_double_source_row_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_guarded_const_backed_double_root_double_source_row_slot_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_double_root_double_source_row_slot_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_guarded_const_backed_triple_root_double_source_row_slot_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_double_source_row_slot_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t40_triple_source_row_slot_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_row_slot_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t41_tail_alias_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_task_handle_forwarded_nested_array_repackage_spawn.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t42_forwarded_alias_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_row_slot_tail_alias_sourced_composed_dynamic_task_handle_forwarded_alias_nested_array_repackage_spawn.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t43_tail_queued_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_queued_spawn.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t44_queue_root_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_root_spawn.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t45_queue_root_alias_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_root_alias_spawn.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t46_queue_root_chain_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_root_chain_spawn.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t47_queue_local_alias_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_local_alias_spawn.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t48_queue_local_chain_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_local_chain_spawn.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t49_queue_local_forward_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_local_forward_spawn.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t50_queue_local_inline_forward_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_local_inline_forward_spawn.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t51_bundle_inline_forward_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_bundle_inline_forward_spawn.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t52_bundle_slot_inline_forward_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_bundle_slot_inline_forward_spawn.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t53_tail_inline_forward_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_direct_inline_forward_spawn.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t54_tail_inline_forward_await_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_direct_inline_forward_await.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t55_bundle_slot_inline_forward_await_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_bundle_slot_inline_forward_await.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t56_bundle_inline_forward_await_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_bundle_inline_forward_await.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t57_queue_local_inline_forward_await_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_local_inline_forward_await.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t58_queue_local_forward_await_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_local_forward_await.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t59_queue_root_inline_forward_await_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_root_inline_forward_await.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t60_queue_root_forward_await_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_root_forward_await.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t61_queue_root_alias_forward_await_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_root_alias_forward_await.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t62_queue_root_chain_forward_await_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_root_chain_forward_await.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t63_queue_root_alias_inline_forward_await_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_root_alias_inline_forward_await.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t64_queue_root_chain_inline_forward_await_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_queue_root_chain_inline_forward_await.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t65_bundle_forward_await_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_bundle_forward_await.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t66_bundle_alias_forward_await_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_bundle_alias_forward_await.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t67_bundle_chain_forward_await_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_bundle_chain_forward_await.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t68_bundle_alias_inline_forward_await_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_bundle_alias_inline_forward_await.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t69_bundle_chain_inline_forward_await_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_bundle_chain_inline_forward_await.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t70_bundle_forward_spawn_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_bundle_forward_spawn.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t71_bundle_alias_forward_spawn_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_bundle_alias_forward_spawn.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_t72_bundle_chain_forward_spawn_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_guarded_const_backed_triple_root_triple_source_tail_bundle_chain_forward_spawn.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_aliased_guard_refined_const_backed_projected_root_task_handle_nested_repackage_reinit_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_aliased_guard_refined_const_backed_projected_root_task_handle_nested_repackage_reinit.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_aliased_guard_refined_const_backed_projected_root_task_handle_nested_repackage_spawn_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_aliased_guard_refined_const_backed_projected_root_task_handle_nested_repackage_spawn.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_aliased_guard_refined_const_backed_projected_root_task_handle_array_repackage_spawn_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_aliased_guard_refined_const_backed_projected_root_task_handle_array_repackage_spawn.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
    ]
}

fn dynamic_task_handle_fail_cases() -> Vec<FailCase> {
    vec![
        FailCase {
            name: "aliased_direct_task_handle_use_after_move_build",
            source_relative: "tests/codegen/fail/aliased_direct_task_handle_use_after_move_build.ql",
            emit: "staticlib",
            expected_stderr_relative: "tests/codegen/fail/aliased_direct_task_handle_use_after_move_build.stderr",
            extra_args: &[],
        },
        FailCase {
            name: "aliased_direct_task_handle_tuple_repackage_use_after_move_build",
            source_relative: "tests/codegen/fail/aliased_direct_task_handle_tuple_repackage_use_after_move_build.ql",
            emit: "staticlib",
            expected_stderr_relative: "tests/codegen/fail/aliased_direct_task_handle_tuple_repackage_use_after_move_build.stderr",
            extra_args: &[],
        },
        FailCase {
            name: "dynamic_task_array_index_assignment_after_consume_build",
            source_relative: "tests/codegen/fail/dynamic_task_array_index_assignment_after_consume_build.ql",
            emit: "staticlib",
            expected_stderr_relative: "tests/codegen/fail/dynamic_task_array_index_assignment_after_consume_build.stderr",
            extra_args: &[],
        },
        FailCase {
            name: "aliased_dynamic_task_handle_root_use_after_move_build",
            source_relative: "tests/codegen/fail/aliased_dynamic_task_handle_root_use_after_move_build.ql",
            emit: "staticlib",
            expected_stderr_relative: "tests/codegen/fail/aliased_dynamic_task_handle_root_use_after_move_build.stderr",
            extra_args: &[],
        },
        FailCase {
            name: "aliased_dynamic_task_handle_root_tuple_repackage_use_after_move_build",
            source_relative: "tests/codegen/fail/aliased_dynamic_task_handle_root_tuple_repackage_use_after_move_build.ql",
            emit: "staticlib",
            expected_stderr_relative: "tests/codegen/fail/aliased_dynamic_task_handle_root_tuple_repackage_use_after_move_build.stderr",
            extra_args: &[],
        },
        FailCase {
            name: "projected_root_const_dynamic_task_handle_use_after_move_build",
            source_relative: "tests/codegen/fail/projected_root_const_dynamic_task_handle_use_after_move_build.ql",
            emit: "staticlib",
            expected_stderr_relative: "tests/codegen/fail/projected_root_const_dynamic_task_handle_use_after_move_build.stderr",
            extra_args: &[],
        },
        FailCase {
            name: "composed_dynamic_task_handle_use_after_move_build",
            source_relative: "tests/codegen/fail/composed_dynamic_task_handle_use_after_move_build.ql",
            emit: "staticlib",
            expected_stderr_relative: "tests/codegen/fail/composed_dynamic_task_handle_use_after_move_build.stderr",
            extra_args: &[],
        },
        FailCase {
            name: "alias_sourced_composed_dynamic_task_handle_use_after_move_build",
            source_relative: "tests/codegen/fail/alias_sourced_composed_dynamic_task_handle_use_after_move_build.ql",
            emit: "staticlib",
            expected_stderr_relative: "tests/codegen/fail/alias_sourced_composed_dynamic_task_handle_use_after_move_build.stderr",
            extra_args: &[],
        },
    ]
}

fn dynamic_task_handle_pass_cases() -> Vec<PassCase> {
    vec![
        PassCase {
            name: "async_program_main_dynamic_task_handle_array_assignment_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_dynamic_task_handle_array_assignment.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_dynamic_task_handle_spawn_sibling_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_dynamic_task_handle_spawn_sibling.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_aliased_direct_task_handle_reinit_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_aliased_direct_task_handle_reinit.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_composed_dynamic_task_handle_reinit_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_composed_dynamic_task_handle_reinit.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_program_main_alias_sourced_composed_dynamic_task_handle_reinit_exe",
            source_relative: "fixtures/codegen/pass/async_program_main_alias_sourced_composed_dynamic_task_handle_reinit.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
    ]
}

fn run_pass_case(workspace_root: &Path, case: &PassCase) -> Result<(), String> {
    let temp = TempDir::new(&format!("ql-codegen-{}", case.name));
    let output_path = artifact_output_path(temp.path(), case.emit);
    let expected_path = workspace_root.join(case.expected_relative);
    let expected = normalize_trimmed(&render_expected_snapshot(&read_normalized_file(
        &expected_path,
        "expected snapshot",
    )));

    let mut command = ql_command(workspace_root);
    command.args([
        "build",
        case.source_relative,
        "--emit",
        case.emit,
        "--output",
        &output_path.to_string_lossy(),
    ]);
    if let Some(surface) = case.header_surface {
        if surface == "exports" {
            command.arg("--header");
        } else {
            command.args(["--header-surface", surface]);
        }
    }

    let mut compiler_wrapper = None;
    if case.mock_compiler {
        compiler_wrapper = Some(make_mock_compiler_wrapper(temp.path()));
    }
    if let Some(wrapper) = &compiler_wrapper {
        command.env("QLANG_CLANG", wrapper);
    }

    let mut archiver_wrapper = None;
    if case.mock_archiver {
        archiver_wrapper = Some(make_mock_archiver_wrapper(temp.path()));
    }
    if let Some(wrapper) = &archiver_wrapper {
        command.env("QLANG_AR", wrapper);
    }
    if let Some(style) = case.archiver_style {
        command.env("QLANG_AR_STYLE", style);
    }

    let output = run_command_capture(
        &mut command,
        format!("`ql build {} --emit {}`", case.source_relative, case.emit),
    );
    let (_, stderr) = expect_success(case.name, "successful build", &output)?;
    expect_empty_stderr(case.name, "successful build", &stderr)?;
    expect_file_exists(
        case.name,
        &output_path,
        "generated artifact",
        "successful build",
    )?;

    let actual = read_normalized_trimmed_file(&output_path, "generated artifact");
    expect_snapshot_matches(case.name, "artifact snapshot", &expected, &actual)?;

    if let Some(expected_header_relative) = case.expected_header_relative {
        let expected_header_path = workspace_root.join(expected_header_relative);
        let expected_header =
            read_normalized_trimmed_file(&expected_header_path, "expected header snapshot");
        let surface = case
            .header_surface
            .expect("header snapshots require an explicit surface");
        let header_output_path =
            default_sidecar_header_output_path(&output_path, case.source_relative, surface);
        expect_file_exists(
            case.name,
            &header_output_path,
            "generated header",
            "successful build",
        )?;
        let actual_header = read_normalized_trimmed_file(&header_output_path, "generated header");
        expect_snapshot_matches(
            case.name,
            "header snapshot",
            &expected_header,
            &actual_header,
        )?;
    }

    let leftovers = fs::read_dir(temp.path())
        .unwrap_or_else(|_| panic!("read temp dir `{}`", temp.path().display()))
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.contains(".codegen."))
        })
        .collect::<Vec<_>>();
    if !leftovers.is_empty() {
        let rendered = leftovers
            .into_iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .join(", ");
        return Err(format!(
            "[{}] expected no preserved intermediate artifacts on success, found: {}",
            case.name, rendered
        ));
    }

    Ok(())
}

fn run_fail_case(workspace_root: &Path, case: &FailCase) -> Result<(), String> {
    let expected_path = workspace_root.join(case.expected_stderr_relative);
    let expected = read_normalized_file(&expected_path, "expected stderr snapshot");

    let mut command = ql_command(workspace_root);
    command.args(["build", case.source_relative, "--emit", case.emit]);
    command.args(case.extra_args);
    let output = run_command_capture(
        &mut command,
        format!("`ql build {} --emit {}`", case.source_relative, case.emit),
    );
    let (stdout, stderr) = expect_exit_code(case.name, "failing build", &output, 1)?;
    expect_empty_stdout(case.name, "failing build", &stdout)?;

    expect_snapshot_matches(case.name, "stderr snapshot", &expected, &stderr)?;

    Ok(())
}

fn artifact_output_path(root: &Path, emit: &str) -> PathBuf {
    match emit {
        "llvm-ir" => root.join("artifact.ll"),
        "obj" => root.join(if cfg!(windows) {
            "artifact.obj"
        } else {
            "artifact.o"
        }),
        "exe" => root.join(if cfg!(windows) {
            "artifact.exe"
        } else {
            "artifact"
        }),
        "dylib" => root.join(if cfg!(windows) {
            "artifact.dll"
        } else if cfg!(target_os = "macos") {
            "libartifact.dylib"
        } else {
            "libartifact.so"
        }),
        "staticlib" => root.join(if cfg!(windows) {
            "artifact.lib"
        } else {
            "libartifact.a"
        }),
        other => panic!("unsupported emit kind `{other}`"),
    }
}

fn default_sidecar_header_output_path(
    artifact_path: &Path,
    source_relative: &str,
    surface: &str,
) -> PathBuf {
    let stem = Path::new(source_relative)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .unwrap_or("module");
    let file_name = match surface {
        "exports" => format!("{stem}.h"),
        "imports" => format!("{stem}.imports.h"),
        "both" => format!("{stem}.ffi.h"),
        other => panic!("unsupported header surface `{other}`"),
    };

    artifact_path
        .parent()
        .expect("artifact output should have a parent directory")
        .join(file_name)
}

fn render_expected_snapshot(snapshot: &str) -> String {
    snapshot.replace("{{TARGET_TRIPLE}}", current_target_triple())
}

fn current_target_triple() -> &'static str {
    match (env::consts::ARCH, env::consts::OS) {
        ("x86_64", "windows") => "x86_64-pc-windows-msvc",
        ("x86_64", "linux") => "x86_64-pc-linux-gnu",
        ("aarch64", "macos") => "aarch64-apple-darwin",
        ("x86_64", "macos") => "x86_64-apple-darwin",
        ("aarch64", "linux") => "aarch64-unknown-linux-gnu",
        _ => "unknown-unknown-unknown",
    }
}

fn current_archiver_style() -> &'static str {
    if cfg!(windows) { "lib" } else { "ar" }
}

fn make_mock_compiler_wrapper(root: &Path) -> PathBuf {
    if cfg!(windows) {
        let script = root.join("mock-clang.ps1");
        fs::write(
            &script,
            r#"
param([string[]]$args)
$out = $null
$isCompile = $false
$isShared = $false
for ($i = 0; $i -lt $args.Count; $i++) {
    if ($args[$i] -eq '-c') { $isCompile = $true }
    if ($args[$i] -eq '-shared' -or $args[$i] -eq '-dynamiclib') { $isShared = $true }
    if ($args[$i] -eq '-o') { $out = $args[$i + 1] }
}
if ($null -eq $out) { Write-Error 'missing -o'; exit 1 }
if ($isCompile) {
    Set-Content -Path $out -NoNewline -Value 'mock-object'
} elseif ($isShared) {
    Set-Content -Path $out -NoNewline -Value 'mock-dylib'
} else {
    Set-Content -Path $out -NoNewline -Value 'mock-executable'
}
"#,
        )
        .expect("write mock clang powershell script");
        let wrapper = root.join("mock-clang.cmd");
        fs::write(
            &wrapper,
            format!(
                "@echo off\r\npowershell.exe -ExecutionPolicy Bypass -File \"{}\" %*\r\n",
                script.display()
            ),
        )
        .expect("write mock clang wrapper");
        wrapper
    } else {
        let script = root.join("mock-clang.sh");
        fs::write(
            &script,
            r#"#!/bin/sh
out=""
is_compile=0
is_shared=0
while [ "$#" -gt 0 ]; do
  if [ "$1" = "-c" ]; then
    is_compile=1
    shift
    continue
  fi
  if [ "$1" = "-shared" ] || [ "$1" = "-dynamiclib" ]; then
    is_shared=1
    shift
    continue
  fi
  if [ "$1" = "-o" ]; then
    out="$2"
    shift 2
    continue
  fi
  shift
done
if [ "$out" = "" ]; then
  echo "missing -o" 1>&2
  exit 1
fi
if [ "$is_compile" -eq 1 ]; then
  printf 'mock-object' > "$out"
elif [ "$is_shared" -eq 1 ]; then
  printf 'mock-dylib' > "$out"
else
  printf 'mock-executable' > "$out"
fi
"#,
        )
        .expect("write mock clang shell script");
        make_executable(&script);
        script
    }
}

fn make_mock_archiver_wrapper(root: &Path) -> PathBuf {
    if cfg!(windows) {
        let script = root.join("mock-archiver.ps1");
        fs::write(
            &script,
            r#"
param([string[]]$args)
$out = $null
for ($i = 0; $i -lt $args.Count; $i++) {
    if ($args[$i] -like '/OUT:*') { $out = $args[$i].Substring(5) }
}
if ($null -eq $out) { Write-Error 'missing /OUT'; exit 1 }
Set-Content -Path $out -NoNewline -Value 'mock-staticlib'
"#,
        )
        .expect("write mock archiver powershell script");
        let wrapper = root.join("mock-archiver.cmd");
        fs::write(
            &wrapper,
            format!(
                "@echo off\r\npowershell.exe -ExecutionPolicy Bypass -File \"{}\" %*\r\n",
                script.display()
            ),
        )
        .expect("write mock archiver wrapper");
        wrapper
    } else {
        let script = root.join("mock-archiver.sh");
        fs::write(
            &script,
            r#"#!/bin/sh
out="$2"
printf 'mock-staticlib' > "$out"
"#,
        )
        .expect("write mock archiver shell script");
        make_executable(&script);
        script
    }
}

#[cfg(unix)]
fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = fs::metadata(path)
        .unwrap_or_else(|_| panic!("read metadata for `{}`", path.display()))
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)
        .unwrap_or_else(|_| panic!("set executable bit on `{}`", path.display()));
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) {}
