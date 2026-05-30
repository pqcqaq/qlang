mod support;

use support::{
    TempDir, expect_exit_code, expect_stderr_contains, expect_stderr_not_contains, ql_command,
    run_command_capture, workspace_root,
};

#[test]
fn check_package_dir_preserves_invalid_manifest_rerun_hint() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-package-invalid-manifest");
    let app_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(&app_root).expect("create package directory for invalid manifest test");

    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package
name = "app"
"#,
    );
    temp.write(
        "workspace/app/src/lib.ql",
        r#"
package demo.app

pub fn main() -> Int {
    return 1
}
"#,
    );

    let mut command = ql_command(&workspace_root);
    command.args(["check"]).arg(&app_root);
    let output = run_command_capture(&mut command, "`ql check` package invalid manifest");
    let (_stdout, stderr) = expect_exit_code(
        "project-check-package-invalid-manifest",
        "direct package ql check with invalid manifest",
        &output,
        1,
    )
    .expect("direct package ql check with invalid manifest should fail");
    let normalized_stderr = stderr.replace('\\', "/");
    let manifest_display = app_root
        .join("qlang.toml")
        .display()
        .to_string()
        .replace('\\', "/");
    let error_line = format!("error: `ql check` invalid manifest `{manifest_display}`");
    let old_error_line = format!("error: invalid manifest `{manifest_display}`");
    let package_note = format!("note: failing package manifest: {manifest_display}");
    let rerun_hint =
        format!("hint: rerun `ql check {manifest_display}` after fixing the package manifest");
    expect_stderr_contains(
        "project-check-package-invalid-manifest",
        "direct package ql check with invalid manifest",
        &normalized_stderr,
        &error_line,
    )
    .expect("direct package ql check should preserve the command label for invalid manifests");
    expect_stderr_not_contains(
        "project-check-package-invalid-manifest",
        "direct package ql check with invalid manifest",
        &normalized_stderr,
        &old_error_line,
    )
    .expect("direct package ql check should not fall back to the generic invalid manifest error");
    expect_stderr_contains(
        "project-check-package-invalid-manifest",
        "direct package ql check with invalid manifest",
        &normalized_stderr,
        &package_note,
    )
    .expect("direct package ql check should point to the failing package manifest");
    expect_stderr_contains(
        "project-check-package-invalid-manifest",
        "direct package ql check with invalid manifest",
        &normalized_stderr,
        &rerun_hint,
    )
    .expect("direct package ql check should suggest rerunning the same manifest path");
}

#[test]
fn check_package_dir_preserves_missing_package_name_rerun_hint() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-package-missing-package-name");
    let app_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(&app_root)
        .expect("create package directory for missing package name test");

    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
version = "0.1.0"
"#,
    );
    temp.write(
        "workspace/app/src/lib.ql",
        r#"
package demo.app

pub fn main() -> Int {
    return 1
}
"#,
    );

    let mut command = ql_command(&workspace_root);
    command.args(["check"]).arg(&app_root);
    let output = run_command_capture(&mut command, "`ql check` package missing package name");
    let (_stdout, stderr) = expect_exit_code(
        "project-check-package-missing-package-name",
        "direct package ql check with missing package name",
        &output,
        1,
    )
    .expect("direct package ql check with missing package name should fail");
    let normalized_stderr = stderr.replace('\\', "/");
    let manifest_display = app_root
        .join("qlang.toml")
        .display()
        .to_string()
        .replace('\\', "/");
    let error_line = format!(
        "error: `ql check` manifest `{manifest_display}` does not declare `[package].name`"
    );
    let old_error_line = format!(
        "error: `ql check` invalid manifest `{manifest_display}`: `[package].name` must be present"
    );
    let package_note = format!("note: failing package manifest: {manifest_display}");
    let rerun_hint =
        format!("hint: rerun `ql check {manifest_display}` after fixing the package manifest");
    expect_stderr_contains(
        "project-check-package-missing-package-name",
        "direct package ql check with missing package name",
        &normalized_stderr,
        &error_line,
    )
    .expect("direct package ql check should preserve the command label for missing package names");
    expect_stderr_not_contains(
        "project-check-package-missing-package-name",
        "direct package ql check with missing package name",
        &normalized_stderr,
        &old_error_line,
    )
    .expect(
        "direct package ql check should not fall back to the parse-error missing package-name message",
    );
    let error_line_index = normalized_stderr
        .find(&error_line)
        .expect("direct package ql check should include the missing package-name error");
    let package_note_index = normalized_stderr
        .find(&package_note)
        .expect("direct package ql check should include the package manifest note");
    let rerun_hint_index = normalized_stderr
        .find(&rerun_hint)
        .expect("direct package ql check should include the rerun hint");
    assert!(
        error_line_index < package_note_index && package_note_index < rerun_hint_index,
        "expected missing package-name context before rerun hint, got:\n{stderr}"
    );
}

#[test]
fn check_package_dir_sync_interfaces_preserves_invalid_manifest_rerun_hint() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-sync-invalid-manifest");
    let app_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(&app_root)
        .expect("create package directory for sync invalid manifest test");

    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package
name = "app"
"#,
    );
    temp.write(
        "workspace/app/src/lib.ql",
        r#"
package demo.app

pub fn main() -> Int {
    return 1
}
"#,
    );

    let mut command = ql_command(&workspace_root);
    command.args(["check", "--sync-interfaces"]).arg(&app_root);
    let output = run_command_capture(
        &mut command,
        "`ql check --sync-interfaces` package invalid manifest",
    );
    let (_stdout, stderr) = expect_exit_code(
        "project-check-sync-invalid-manifest",
        "direct package ql check sync with invalid manifest",
        &output,
        1,
    )
    .expect("direct package ql check sync with invalid manifest should fail");
    let normalized_stderr = stderr.replace('\\', "/");
    let manifest_display = app_root
        .join("qlang.toml")
        .display()
        .to_string()
        .replace('\\', "/");
    let error_line =
        format!("error: `ql check --sync-interfaces` invalid manifest `{manifest_display}`");
    let old_error_line = format!("error: invalid manifest `{manifest_display}`");
    let package_note = format!("note: failing package manifest: {manifest_display}");
    let rerun_hint = format!(
        "hint: rerun `ql check --sync-interfaces {manifest_display}` after fixing the package manifest"
    );
    expect_stderr_contains(
        "project-check-sync-invalid-manifest",
        "direct package ql check sync with invalid manifest",
        &normalized_stderr,
        &error_line,
    )
    .expect("direct package ql check sync should preserve the command label for invalid manifests");
    expect_stderr_not_contains(
        "project-check-sync-invalid-manifest",
        "direct package ql check sync with invalid manifest",
        &normalized_stderr,
        &old_error_line,
    )
    .expect(
        "direct package ql check sync should not fall back to the generic invalid manifest error",
    );
    expect_stderr_contains(
        "project-check-sync-invalid-manifest",
        "direct package ql check sync with invalid manifest",
        &normalized_stderr,
        &package_note,
    )
    .expect("direct package ql check sync should point to the failing package manifest");
    expect_stderr_contains(
        "project-check-sync-invalid-manifest",
        "direct package ql check sync with invalid manifest",
        &normalized_stderr,
        &rerun_hint,
    )
    .expect("direct package ql check sync should suggest rerunning the same manifest path");
}

#[test]
fn check_package_dir_sync_interfaces_preserves_missing_package_name_rerun_hint() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-sync-missing-package-name");
    let app_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(&app_root)
        .expect("create package directory for sync missing package name test");

    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
version = "0.1.0"
"#,
    );
    temp.write(
        "workspace/app/src/lib.ql",
        r#"
package demo.app

pub fn main() -> Int {
    return 1
}
"#,
    );

    let mut command = ql_command(&workspace_root);
    command.args(["check", "--sync-interfaces"]).arg(&app_root);
    let output = run_command_capture(
        &mut command,
        "`ql check --sync-interfaces` package missing package name",
    );
    let (_stdout, stderr) = expect_exit_code(
        "project-check-sync-missing-package-name",
        "direct package ql check sync with missing package name",
        &output,
        1,
    )
    .expect("direct package ql check sync with missing package name should fail");
    let normalized_stderr = stderr.replace('\\', "/");
    let manifest_display = app_root
        .join("qlang.toml")
        .display()
        .to_string()
        .replace('\\', "/");
    let error_line = format!(
        "error: `ql check --sync-interfaces` manifest `{manifest_display}` does not declare `[package].name`"
    );
    let old_error_line = format!(
        "error: `ql check --sync-interfaces` invalid manifest `{manifest_display}`: `[package].name` must be present"
    );
    let package_note = format!("note: failing package manifest: {manifest_display}");
    let rerun_hint = format!(
        "hint: rerun `ql check --sync-interfaces {manifest_display}` after fixing the package manifest"
    );
    expect_stderr_contains(
        "project-check-sync-missing-package-name",
        "direct package ql check sync with missing package name",
        &normalized_stderr,
        &error_line,
    )
    .expect(
        "direct package ql check sync should preserve the command label for missing package names",
    );
    expect_stderr_not_contains(
        "project-check-sync-missing-package-name",
        "direct package ql check sync with missing package name",
        &normalized_stderr,
        &old_error_line,
    )
    .expect(
        "direct package ql check sync should not fall back to the parse-error missing package-name message",
    );
    let error_line_index = normalized_stderr
        .find(&error_line)
        .expect("direct package ql check sync should include the missing package-name error");
    let package_note_index = normalized_stderr
        .find(&package_note)
        .expect("direct package ql check sync should include the package manifest note");
    let rerun_hint_index = normalized_stderr
        .find(&rerun_hint)
        .expect("direct package ql check sync should include the rerun hint");
    assert!(
        error_line_index < package_note_index && package_note_index < rerun_hint_index,
        "expected sync missing package-name context before rerun hint, got:\n{stderr}"
    );
}

#[test]
fn check_package_dir_preserves_missing_source_root_rerun_hint() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-package-missing-source-root");
    let app_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(&app_root)
        .expect("create package directory for missing source root test");

    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );

    let mut command = ql_command(&workspace_root);
    command.args(["check"]).arg(&app_root);
    let output = run_command_capture(&mut command, "`ql check` package missing source root");
    let (_stdout, stderr) = expect_exit_code(
        "project-check-package-missing-source-root",
        "direct package ql check with missing source root",
        &output,
        1,
    )
    .expect("direct package ql check with missing source root should fail");
    let normalized_stderr = stderr.replace('\\', "/");
    let manifest_display = app_root
        .join("qlang.toml")
        .display()
        .to_string()
        .replace('\\', "/");
    let source_root_display = app_root
        .join("src")
        .display()
        .to_string()
        .replace('\\', "/");
    let error_line = format!(
        "error: `ql check` package source directory `{source_root_display}` does not exist"
    );
    let old_error_line =
        format!("error: package source directory `{source_root_display}` does not exist");
    let package_note = format!("note: failing package manifest: {manifest_display}");
    let source_root_note = format!("note: failing package source root: {source_root_display}");
    let rerun_hint =
        format!("hint: rerun `ql check {manifest_display}` after fixing the package source root");
    expect_stderr_contains(
        "project-check-package-missing-source-root",
        "direct package ql check with missing source root",
        &normalized_stderr,
        &error_line,
    )
    .expect("direct package ql check should preserve the command label for missing source roots");
    expect_stderr_not_contains(
        "project-check-package-missing-source-root",
        "direct package ql check with missing source root",
        &normalized_stderr,
        &old_error_line,
    )
    .expect(
        "direct package ql check should not fall back to the generic missing source-root error",
    );
    expect_stderr_contains(
        "project-check-package-missing-source-root",
        "direct package ql check with missing source root",
        &normalized_stderr,
        &package_note,
    )
    .expect("direct package ql check should point to the failing package manifest");
    expect_stderr_contains(
        "project-check-package-missing-source-root",
        "direct package ql check with missing source root",
        &normalized_stderr,
        &source_root_note,
    )
    .expect("direct package ql check should point to the missing source root");
    expect_stderr_contains(
        "project-check-package-missing-source-root",
        "direct package ql check with missing source root",
        &normalized_stderr,
        &rerun_hint,
    )
    .expect("direct package ql check should suggest rerunning the same manifest path");
}

#[test]
fn check_package_dir_sync_interfaces_preserves_missing_source_root_rerun_hint() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-sync-missing-source-root");
    let app_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(&app_root)
        .expect("create package directory for sync missing source root test");

    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );

    let mut command = ql_command(&workspace_root);
    command.args(["check", "--sync-interfaces"]).arg(&app_root);
    let output = run_command_capture(
        &mut command,
        "`ql check --sync-interfaces` package missing source root",
    );
    let (_stdout, stderr) = expect_exit_code(
        "project-check-sync-missing-source-root",
        "direct package ql check sync with missing source root",
        &output,
        1,
    )
    .expect("direct package ql check sync with missing source root should fail");
    let normalized_stderr = stderr.replace('\\', "/");
    let manifest_display = app_root
        .join("qlang.toml")
        .display()
        .to_string()
        .replace('\\', "/");
    let source_root_display = app_root
        .join("src")
        .display()
        .to_string()
        .replace('\\', "/");
    let error_line = format!(
        "error: `ql check --sync-interfaces` package source directory `{source_root_display}` does not exist"
    );
    let old_error_line =
        format!("error: package source directory `{source_root_display}` does not exist");
    let package_note = format!("note: failing package manifest: {manifest_display}");
    let source_root_note = format!("note: failing package source root: {source_root_display}");
    let rerun_hint = format!(
        "hint: rerun `ql check --sync-interfaces {manifest_display}` after fixing the package source root"
    );
    expect_stderr_contains(
        "project-check-sync-missing-source-root",
        "direct package ql check sync with missing source root",
        &normalized_stderr,
        &error_line,
    )
    .expect(
        "direct package ql check sync should preserve the command label for missing source roots",
    );
    expect_stderr_not_contains(
        "project-check-sync-missing-source-root",
        "direct package ql check sync with missing source root",
        &normalized_stderr,
        &old_error_line,
    )
    .expect(
        "direct package ql check sync should not fall back to the generic missing source-root error",
    );
    expect_stderr_contains(
        "project-check-sync-missing-source-root",
        "direct package ql check sync with missing source root",
        &normalized_stderr,
        &package_note,
    )
    .expect("direct package ql check sync should point to the failing package manifest");
    expect_stderr_contains(
        "project-check-sync-missing-source-root",
        "direct package ql check sync with missing source root",
        &normalized_stderr,
        &source_root_note,
    )
    .expect("direct package ql check sync should point to the missing source root");
    expect_stderr_contains(
        "project-check-sync-missing-source-root",
        "direct package ql check sync with missing source root",
        &normalized_stderr,
        &rerun_hint,
    )
    .expect("direct package ql check sync should suggest rerunning the same manifest path");
}

#[test]
fn check_package_dir_preserves_empty_source_root_rerun_hint() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-package-empty-source-root");
    let app_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(app_root.join("src"))
        .expect("create package source root for empty source root test");

    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );

    let mut command = ql_command(&workspace_root);
    command.args(["check"]).arg(&app_root);
    let output = run_command_capture(&mut command, "`ql check` package empty source root");
    let (_stdout, stderr) = expect_exit_code(
        "project-check-package-empty-source-root",
        "direct package ql check with empty source root",
        &output,
        1,
    )
    .expect("direct package ql check with empty source root should fail");
    let normalized_stderr = stderr.replace('\\', "/");
    let manifest_display = app_root
        .join("qlang.toml")
        .display()
        .to_string()
        .replace('\\', "/");
    let source_root_display = app_root
        .join("src")
        .display()
        .to_string()
        .replace('\\', "/");
    let error_line =
        format!("error: `ql check` no `.ql` files found under `{source_root_display}`");
    let old_error_line = format!("error: no `.ql` files found under `{source_root_display}`");
    let package_note = format!("note: failing package manifest: {manifest_display}");
    let source_root_note = format!("note: failing package source root: {source_root_display}");
    let rerun_hint =
        format!("hint: rerun `ql check {manifest_display}` after adding package source files");
    expect_stderr_contains(
        "project-check-package-empty-source-root",
        "direct package ql check with empty source root",
        &normalized_stderr,
        &error_line,
    )
    .expect("direct package ql check should preserve the command label for empty source roots");
    expect_stderr_not_contains(
        "project-check-package-empty-source-root",
        "direct package ql check with empty source root",
        &normalized_stderr,
        &old_error_line,
    )
    .expect("direct package ql check should not fall back to the generic empty source-root error");
    expect_stderr_contains(
        "project-check-package-empty-source-root",
        "direct package ql check with empty source root",
        &normalized_stderr,
        &package_note,
    )
    .expect("direct package ql check should point to the failing package manifest");
    expect_stderr_contains(
        "project-check-package-empty-source-root",
        "direct package ql check with empty source root",
        &normalized_stderr,
        &source_root_note,
    )
    .expect("direct package ql check should point to the empty source root");
    expect_stderr_contains(
        "project-check-package-empty-source-root",
        "direct package ql check with empty source root",
        &normalized_stderr,
        &rerun_hint,
    )
    .expect("direct package ql check should suggest rerunning the same manifest path");
}

#[test]
fn check_package_dir_sync_interfaces_preserves_empty_source_root_rerun_hint() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-sync-empty-source-root");
    let app_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(app_root.join("src"))
        .expect("create package source root for sync empty source root test");

    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );

    let mut command = ql_command(&workspace_root);
    command.args(["check", "--sync-interfaces"]).arg(&app_root);
    let output = run_command_capture(
        &mut command,
        "`ql check --sync-interfaces` package empty source root",
    );
    let (_stdout, stderr) = expect_exit_code(
        "project-check-sync-empty-source-root",
        "direct package ql check sync with empty source root",
        &output,
        1,
    )
    .expect("direct package ql check sync with empty source root should fail");
    let normalized_stderr = stderr.replace('\\', "/");
    let manifest_display = app_root
        .join("qlang.toml")
        .display()
        .to_string()
        .replace('\\', "/");
    let source_root_display = app_root
        .join("src")
        .display()
        .to_string()
        .replace('\\', "/");
    let error_line = format!(
        "error: `ql check --sync-interfaces` no `.ql` files found under `{source_root_display}`"
    );
    let old_error_line = format!("error: no `.ql` files found under `{source_root_display}`");
    let package_note = format!("note: failing package manifest: {manifest_display}");
    let source_root_note = format!("note: failing package source root: {source_root_display}");
    let rerun_hint = format!(
        "hint: rerun `ql check --sync-interfaces {manifest_display}` after adding package source files"
    );
    expect_stderr_contains(
        "project-check-sync-empty-source-root",
        "direct package ql check sync with empty source root",
        &normalized_stderr,
        &error_line,
    )
    .expect(
        "direct package ql check sync should preserve the command label for empty source roots",
    );
    expect_stderr_not_contains(
        "project-check-sync-empty-source-root",
        "direct package ql check sync with empty source root",
        &normalized_stderr,
        &old_error_line,
    )
    .expect(
        "direct package ql check sync should not fall back to the generic empty source-root error",
    );
    expect_stderr_contains(
        "project-check-sync-empty-source-root",
        "direct package ql check sync with empty source root",
        &normalized_stderr,
        &package_note,
    )
    .expect("direct package ql check sync should point to the failing package manifest");
    expect_stderr_contains(
        "project-check-sync-empty-source-root",
        "direct package ql check sync with empty source root",
        &normalized_stderr,
        &source_root_note,
    )
    .expect("direct package ql check sync should point to the empty source root");
    expect_stderr_contains(
        "project-check-sync-empty-source-root",
        "direct package ql check sync with empty source root",
        &normalized_stderr,
        &rerun_hint,
    )
    .expect("direct package ql check sync should suggest rerunning the same manifest path");
}

#[test]
fn check_package_dir_preserves_source_diagnostic_rerun_hint() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-source-diagnostics");
    let app_root = temp.path().join("workspace").join("app");
    let broken_source = app_root.join("src").join("lib.ql");
    std::fs::create_dir_all(app_root.join("src"))
        .expect("create package source root for source diagnostics test");

    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace/app/src/lib.ql",
        r#"
package demo.app

pub fn main( -> Int {
    return 1
}
"#,
    );

    let mut command = ql_command(&workspace_root);
    command.args(["check"]).arg(&app_root);
    let output = run_command_capture(&mut command, "`ql check` package source diagnostics");
    let (_stdout, stderr) = expect_exit_code(
        "project-check-source-diagnostics",
        "direct package ql check with source diagnostics",
        &output,
        1,
    )
    .expect("direct package ql check with source diagnostics should fail");
    let normalized_stderr = stderr.replace('\\', "/");
    let manifest_display = app_root
        .join("qlang.toml")
        .display()
        .to_string()
        .replace('\\', "/");
    let broken_source_line = broken_source.display().to_string().replace('\\', "/");
    let package_note = format!("note: failing package manifest: {manifest_display}");
    let rerun_hint =
        format!("hint: rerun `ql check {manifest_display}` after fixing the package sources");
    expect_stderr_contains(
        "project-check-source-diagnostics",
        "direct package ql check with source diagnostics",
        &normalized_stderr,
        &broken_source_line,
    )
    .expect("direct package ql check should surface the broken source path");
    expect_stderr_contains(
        "project-check-source-diagnostics",
        "direct package ql check with source diagnostics",
        &normalized_stderr,
        &package_note,
    )
    .expect("direct package ql check should point to the failing package manifest");
    expect_stderr_contains(
        "project-check-source-diagnostics",
        "direct package ql check with source diagnostics",
        &normalized_stderr,
        &rerun_hint,
    )
    .expect(
        "direct package ql check should suggest rerunning the same manifest after fixing sources",
    );
    let broken_source_index = normalized_stderr
        .find(&broken_source_line)
        .expect("direct package ql check should include the broken source path");
    let package_note_index = normalized_stderr
        .find(&package_note)
        .expect("direct package ql check should include the package note");
    let rerun_hint_index = normalized_stderr
        .find(&rerun_hint)
        .expect("direct package ql check should include the rerun hint");
    assert!(
        broken_source_index < package_note_index && package_note_index < rerun_hint_index,
        "expected direct package source diagnostics before rerun hint, got:\n{stderr}"
    );
}

#[test]
fn check_package_dir_sync_interfaces_preserves_source_diagnostic_rerun_hint() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-sync-source-diagnostics");
    let app_root = temp.path().join("workspace").join("app");
    let broken_source = app_root.join("src").join("lib.ql");
    std::fs::create_dir_all(app_root.join("src"))
        .expect("create package source root for sync source diagnostics test");

    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace/app/src/lib.ql",
        r#"
package demo.app

pub fn main( -> Int {
    return 1
}
"#,
    );

    let mut command = ql_command(&workspace_root);
    command.args(["check", "--sync-interfaces"]).arg(&app_root);
    let output = run_command_capture(
        &mut command,
        "`ql check --sync-interfaces` package source diagnostics",
    );
    let (_stdout, stderr) = expect_exit_code(
        "project-check-sync-source-diagnostics",
        "direct package ql check sync with source diagnostics",
        &output,
        1,
    )
    .expect("direct package ql check sync with source diagnostics should fail");
    let normalized_stderr = stderr.replace('\\', "/");
    let manifest_display = app_root
        .join("qlang.toml")
        .display()
        .to_string()
        .replace('\\', "/");
    let broken_source_line = broken_source.display().to_string().replace('\\', "/");
    let package_note = format!("note: failing package manifest: {manifest_display}");
    let rerun_hint = format!(
        "hint: rerun `ql check --sync-interfaces {manifest_display}` after fixing the package sources"
    );
    expect_stderr_contains(
        "project-check-sync-source-diagnostics",
        "direct package ql check sync with source diagnostics",
        &normalized_stderr,
        &broken_source_line,
    )
    .expect("direct package ql check sync should surface the broken source path");
    expect_stderr_contains(
        "project-check-sync-source-diagnostics",
        "direct package ql check sync with source diagnostics",
        &normalized_stderr,
        &package_note,
    )
    .expect("direct package ql check sync should point to the failing package manifest");
    expect_stderr_contains(
        "project-check-sync-source-diagnostics",
        "direct package ql check sync with source diagnostics",
        &normalized_stderr,
        &rerun_hint,
    )
    .expect("direct package ql check sync should suggest rerunning the same manifest after fixing sources");
    let broken_source_index = normalized_stderr
        .find(&broken_source_line)
        .expect("direct package ql check sync should include the broken source path");
    let package_note_index = normalized_stderr
        .find(&package_note)
        .expect("direct package ql check sync should include the package note");
    let rerun_hint_index = normalized_stderr
        .find(&rerun_hint)
        .expect("direct package ql check sync should include the rerun hint");
    assert!(
        broken_source_index < package_note_index && package_note_index < rerun_hint_index,
        "expected direct package sync source diagnostics before rerun hint, got:\n{stderr}"
    );
}
