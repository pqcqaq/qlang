mod support;

use support::codegen::{FailCase, PassCase, current_archiver_style, run_fail_case, run_pass_case};
use support::workspace_root;

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

fn dynamic_task_handle_pass_cases() -> Vec<PassCase<'static>> {
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
