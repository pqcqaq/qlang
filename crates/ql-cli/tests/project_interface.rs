mod support;

use support::{
    TempDir, expect_empty_stdout, expect_exit_code, expect_file_exists, expect_snapshot_matches,
    expect_stderr_contains, expect_stderr_not_contains, expect_stdout_contains_all, expect_success,
    ql_command, read_normalized_file, run_command_capture, workspace_root,
};

#[test]
fn project_emit_interface_writes_public_qi_surface() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-interface");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(project_root.join("src").join("nested"))
        .expect("create project source directory for interface emit test");
    let interface_path = project_root.join("app.qi");
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
package demo.api

pub const DEFAULT_PORT: Int = 8080
const INTERNAL_PORT: Int = 3000

pub struct Buffer[T] {
    value: T,
    count: Int = 0,
}

pub trait Writer {
    fn flush(var self) -> Int
}

impl Buffer[Int] {
    pub fn len(self) -> Int {
        return 1
    }

    fn hidden(self) -> Int {
        return 0
    }
}

extend Buffer[Int] {
    pub fn twice(self) -> Int {
        return 2
    }

    fn private_twice(self) -> Int {
        return 1
    }
}

pub fn sum(left: Int, right: Int) -> Int {
    return left + right
}

pub extern "c" {
    fn puts(ptr: *const U8) -> I32
}
"#,
    );
    temp.write(
        "workspace/app/src/nested/types.ql",
        r#"
package demo.api

pub static BUILD_ID: Int = 1

pub enum Shape {
    Unit,
    Pair(Int, Int),
}

pub type Pair = (Int, Int)
"#,
    );

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "emit-interface"])
        .arg(&project_root);
    let output = run_command_capture(&mut command, "`ql project emit-interface`");
    let (stdout, stderr) = expect_success(
        "project-interface-success",
        "project interface emission",
        &output,
    )
    .expect("project interface emission should succeed");
    expect_snapshot_matches(
        "project-interface-success",
        "project interface stdout",
        &format!("wrote interface: {}\n", interface_path.display()),
        &stdout,
    )
    .expect("interface emission should report the written artifact path");
    expect_snapshot_matches(
        "project-interface-success",
        "project interface stderr",
        "",
        &stderr,
    )
    .expect("successful interface emission should stay silent on stderr");
    expect_file_exists(
        "project-interface-success",
        &interface_path,
        "generated interface",
        "project interface emission",
    )
    .expect("interface emission should create the default package qi artifact");

    let expected = "\
// qlang interface v1
// package: app

// source: src/lib.ql
package demo.api

pub const DEFAULT_PORT: Int

pub struct Buffer[T] {
    value: T,
    count: Int,
}

pub trait Writer {
    fn flush(var self) -> Int
}

impl Buffer[Int] {
    pub fn len(self) -> Int
}

extend Buffer[Int] {
    pub fn twice(self) -> Int
}

pub fn sum(left: Int, right: Int) -> Int

pub extern \"c\" {
    fn puts(ptr: *const U8) -> I32
}

// source: src/nested/types.ql
package demo.api

pub static BUILD_ID: Int

pub enum Shape {
    Unit,
    Pair(Int, Int),
}

pub type Pair = (Int, Int)
";
    let actual = read_normalized_file(&interface_path, "generated qi artifact");
    expect_snapshot_matches(
        "project-interface-success",
        "generated qi artifact",
        expected,
        &actual,
    )
    .expect("generated qi artifact should match the public interface snapshot");
}

#[test]
fn project_emit_interface_reports_all_failing_package_sources() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-interface-package-source-failures");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create project source directory for package source failure test");
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
package demo.api

pub fn exported() -> Int {
    return 1
}
"#,
    );
    let first_failure = temp.write(
        "workspace/app/src/a_broken.ql",
        r#"
package demo.api

pub fn broken_first(value: MissingFirst) -> Int {
    return value
}
"#,
    );
    temp.write(
        "workspace/app/src/z_broken.ql",
        r#"
package demo.api

pub fn broken_second(value: MissingSecond) -> Int {
    return value
}
"#,
    );
    let interface_path = project_root.join("app.qi");
    let manifest_path = project_root.join("qlang.toml");
    let first_failure_display = first_failure.to_string_lossy().replace('\\', "/");
    let manifest_display = manifest_path.to_string_lossy().replace('\\', "/");

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "emit-interface"])
        .arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql project emit-interface` package with multiple failing sources",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-interface-package-source-failures",
        "package interface emission with multiple failing sources",
        &output,
        1,
    )
    .expect("package interface emission with multiple failing sources should fail");
    expect_empty_stdout(
        "project-interface-package-source-failures",
        "package interface emission with multiple failing sources",
        &stdout,
    )
    .expect("failing package interface emission should not report a written artifact");
    expect_stderr_contains(
        "project-interface-package-source-failures",
        "package interface emission with multiple failing sources",
        &stderr,
        "a_broken.ql",
    )
    .expect("package interface emission should report the first failing source file");
    expect_stderr_contains(
        "project-interface-package-source-failures",
        "package interface emission with multiple failing sources",
        &stderr,
        "z_broken.ql",
    )
    .expect("package interface emission should continue reporting later failing source files");
    expect_stderr_contains(
        "project-interface-package-source-failures",
        "package interface emission with multiple failing sources",
        &stderr,
        "interface emission found 2 failing source file(s)",
    )
    .expect("package interface emission should summarize all failing source files");
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stderr_contains(
        "project-interface-package-source-failures",
        "package interface emission with multiple failing sources",
        &normalized_stderr,
        &format!("note: first failing source file: {first_failure_display}"),
    )
    .expect("package interface emission should point to the first failing source file");
    expect_stderr_contains(
        "project-interface-package-source-failures",
        "package interface emission with multiple failing sources",
        &normalized_stderr,
        &format!("note: failing package manifest: {manifest_display}"),
    )
    .expect("package interface emission should point to the failing package manifest");
    expect_stderr_contains(
        "project-interface-package-source-failures",
        "package interface emission with multiple failing sources",
        &normalized_stderr,
        &format!(
            "hint: rerun `ql project emit-interface {}` after fixing the package interface error",
            manifest_display
        ),
    )
    .expect("package interface emission should suggest rerunning package interface emission");
    assert!(
        !interface_path.is_file(),
        "failing package interface emission should not create `{}`",
        interface_path.display()
    );
}

#[test]
fn project_emit_interface_dedupes_single_failing_package_source_summary() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-interface-single-package-source-failure");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create project source directory for single package source failure test");
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
package demo.api

pub fn exported() -> Int {
    return 1
}
"#,
    );
    let broken_source = temp.write(
        "workspace/app/src/broken.ql",
        r#"
package demo.api

pub fn broken(value: MissingType) -> Int {
    return value
}
"#,
    );
    let interface_path = project_root.join("app.qi");
    let manifest_path = project_root.join("qlang.toml");
    let normalized_stderr_path = broken_source.to_string_lossy().replace('\\', "/");
    let manifest_display = manifest_path.to_string_lossy().replace('\\', "/");

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "emit-interface"])
        .arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql project emit-interface` package with single failing source",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-interface-single-package-source-failure",
        "package interface emission with single failing source",
        &output,
        1,
    )
    .expect("package interface emission with a single failing source should fail");
    expect_empty_stdout(
        "project-interface-single-package-source-failure",
        "package interface emission with single failing source",
        &stdout,
    )
    .expect("single failing package interface emission should not report a written artifact");
    expect_stderr_contains(
        "project-interface-single-package-source-failure",
        "package interface emission with single failing source",
        &stderr,
        "broken.ql",
    )
    .expect("package interface emission should surface the broken source file");
    expect_stderr_contains(
        "project-interface-single-package-source-failure",
        "package interface emission with single failing source",
        &stderr,
        "interface emission found 1 failing source file(s)",
    )
    .expect("package interface emission should summarize the single failing source");
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stderr_not_contains(
        "project-interface-single-package-source-failure",
        "package interface emission with single failing source",
        &normalized_stderr,
        "note: first failing source file:",
    )
    .expect(
        "single failing package sources should not repeat the source path in the final summary",
    );
    expect_stderr_contains(
        "project-interface-single-package-source-failure",
        "package interface emission with single failing source",
        &normalized_stderr,
        &format!("note: failing package manifest: {manifest_display}"),
    )
    .expect("package interface emission should still point to the failing package manifest");
    expect_stderr_contains(
        "project-interface-single-package-source-failure",
        "package interface emission with single failing source",
        &normalized_stderr,
        &format!(
            "hint: rerun `ql project emit-interface {}` after fixing the package interface error",
            manifest_display
        ),
    )
    .expect("package interface emission should still suggest rerunning package interface emission");
    expect_stderr_contains(
        "project-interface-single-package-source-failure",
        "package interface emission with single failing source",
        &normalized_stderr,
        &normalized_stderr_path,
    )
    .expect("package interface emission should still surface the broken source path locally");
    assert!(
        !interface_path.is_file(),
        "single failing package interface emission should not create `{}`",
        interface_path.display()
    );
}

#[test]
fn project_emit_interface_points_blocked_output_paths_at_interface_target() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-interface-output-path-failure");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create project source directory for blocked output path test");
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

pub fn exported() -> Int {
    return 1
}
"#,
    );
    let interface_path = project_root.join("app.qi");
    std::fs::create_dir_all(&interface_path)
        .expect("create blocking interface directory for emit-interface test");
    let manifest_path = project_root.join("qlang.toml");
    let manifest_display = manifest_path.to_string_lossy().replace('\\', "/");
    let interface_display = interface_path.to_string_lossy().replace('\\', "/");

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "emit-interface"])
        .arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql project emit-interface` package with blocked interface output path",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-interface-output-path-failure",
        "package interface emission with blocked output path",
        &output,
        1,
    )
    .expect("package interface emission should fail when the interface output path is blocked");
    expect_empty_stdout(
        "project-interface-output-path-failure",
        "package interface emission with blocked output path",
        &stdout,
    )
    .expect("blocked output path should not report a written interface");
    expect_stderr_contains(
        "project-interface-output-path-failure",
        "package interface emission with blocked output path",
        &stderr,
        "failed to write interface",
    )
    .expect("blocked output path should surface a write failure");
    let normalized_stderr = stderr.replace('\\', "/");
    let package_note = format!("note: failing package manifest: {manifest_display}");
    let output_note = format!("note: failing interface output path: {interface_display}");
    let rerun_hint = format!(
        "hint: rerun `ql project emit-interface {}` after fixing the interface output path",
        manifest_display
    );
    let old_hint = format!(
        "hint: rerun `ql project emit-interface {}` after fixing the package interface error",
        manifest_display
    );
    expect_stderr_contains(
        "project-interface-output-path-failure",
        "package interface emission with blocked output path",
        &normalized_stderr,
        &package_note,
    )
    .expect("blocked output path should still point to the package manifest");
    expect_stderr_contains(
        "project-interface-output-path-failure",
        "package interface emission with blocked output path",
        &normalized_stderr,
        &output_note,
    )
    .expect("blocked output path should point to the failing interface target");
    expect_stderr_contains(
        "project-interface-output-path-failure",
        "package interface emission with blocked output path",
        &normalized_stderr,
        &rerun_hint,
    )
    .expect("blocked output path should suggest fixing the interface output path");
    expect_stderr_not_contains(
        "project-interface-output-path-failure",
        "package interface emission with blocked output path",
        &normalized_stderr,
        &old_hint,
    )
    .expect("blocked output path should not pretend that package source diagnostics are the issue");
    let package_note_index = normalized_stderr
        .find(&package_note)
        .expect("blocked output path should include the package manifest note");
    let output_note_index = normalized_stderr
        .find(&output_note)
        .expect("blocked output path should include the output-path note");
    let rerun_hint_index = normalized_stderr
        .find(&rerun_hint)
        .expect("blocked output path should include the rerun hint");
    assert!(
        package_note_index < output_note_index && output_note_index < rerun_hint_index,
        "expected blocked output path context before rerun hint, got:\n{stderr}"
    );
    assert!(
        interface_path.is_dir(),
        "blocked output path test should preserve `{}` as a directory",
        interface_path.display()
    );
}

#[test]
fn project_emit_interface_preserves_custom_output_in_source_failure_hints() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-interface-custom-output-source-failure");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create project source directory for custom output source failure test");
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

pub fn exported() -> Int {
    return 1
}
"#,
    );
    temp.write(
        "workspace/app/src/broken.ql",
        r#"
package demo.app

pub fn broken(value: MissingType) -> Int {
    return value
}
"#,
    );
    let output_path = project_root.join("artifacts").join("custom.qi");
    let manifest_path = project_root.join("qlang.toml");
    let manifest_display = manifest_path.to_string_lossy().replace('\\', "/");
    let output_display = output_path.to_string_lossy().replace('\\', "/");

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "emit-interface", "--output"])
        .arg(&output_path)
        .arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql project emit-interface --output` package with source failure",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-interface-custom-output-source-failure",
        "package interface emission with custom output and source failure",
        &output,
        1,
    )
    .expect("custom-output package interface emission should still fail on source errors");
    expect_empty_stdout(
        "project-interface-custom-output-source-failure",
        "package interface emission with custom output and source failure",
        &stdout,
    )
    .expect("source failures should not report a written custom output artifact");
    let normalized_stderr = stderr.replace('\\', "/");
    let rerun_hint = format!(
        "hint: rerun `ql project emit-interface {} --output {}` after fixing the package interface error",
        manifest_display, output_display
    );
    let old_hint = format!(
        "hint: rerun `ql project emit-interface {}` after fixing the package interface error",
        manifest_display
    );
    expect_stderr_contains(
        "project-interface-custom-output-source-failure",
        "package interface emission with custom output and source failure",
        &normalized_stderr,
        &rerun_hint,
    )
    .expect("custom-output source failures should preserve `--output` in the rerun hint");
    expect_stderr_not_contains(
        "project-interface-custom-output-source-failure",
        "package interface emission with custom output and source failure",
        &normalized_stderr,
        &old_hint,
    )
    .expect("custom-output source failures should not drop back to the default rerun hint");
    assert!(
        !output_path.is_file(),
        "custom-output source failure should not create `{}`",
        output_path.display()
    );
}

#[test]
fn project_emit_interface_preserves_custom_output_in_output_path_hints() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-interface-custom-output-path-failure");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create project source directory for custom output path failure test");
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

pub fn exported() -> Int {
    return 1
}
"#,
    );
    let output_path = project_root.join("artifacts").join("custom.qi");
    std::fs::create_dir_all(&output_path)
        .expect("create blocking directory at the custom interface output path");
    let manifest_path = project_root.join("qlang.toml");
    let manifest_display = manifest_path.to_string_lossy().replace('\\', "/");
    let output_display = output_path.to_string_lossy().replace('\\', "/");

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "emit-interface", "--output"])
        .arg(&output_path)
        .arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql project emit-interface --output` package with blocked custom output path",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-interface-custom-output-path-failure",
        "package interface emission with blocked custom output path",
        &output,
        1,
    )
    .expect(
        "custom-output package interface emission should fail when the custom target is blocked",
    );
    expect_empty_stdout(
        "project-interface-custom-output-path-failure",
        "package interface emission with blocked custom output path",
        &stdout,
    )
    .expect("blocked custom output path should not report a written artifact");
    let normalized_stderr = stderr.replace('\\', "/");
    let rerun_hint = format!(
        "hint: rerun `ql project emit-interface {} --output {}` after fixing the interface output path",
        manifest_display, output_display
    );
    let old_hint = format!(
        "hint: rerun `ql project emit-interface {}` after fixing the interface output path",
        manifest_display
    );
    expect_stderr_contains(
        "project-interface-custom-output-path-failure",
        "package interface emission with blocked custom output path",
        &normalized_stderr,
        &rerun_hint,
    )
    .expect("custom-output path failures should preserve `--output` in the rerun hint");
    expect_stderr_not_contains(
        "project-interface-custom-output-path-failure",
        "package interface emission with blocked custom output path",
        &normalized_stderr,
        &old_hint,
    )
    .expect("custom-output path failures should not drop back to the default rerun hint");
    assert!(
        output_path.is_dir(),
        "blocked custom output path test should preserve `{}` as a directory",
        output_path.display()
    );
}

#[test]
fn project_emit_interface_changed_only_skips_up_to_date_package_interface() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-interface-changed-only-package");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create project source directory for changed-only package test");
    let interface_path = project_root.join("app.qi");
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

pub fn exported() -> Int {
    return 1
}
"#,
    );
    let expected = "\
// qlang interface v1
// package: app

// source: src/lib.ql
package demo.app

pub fn exported() -> Int
";
    temp.write("workspace/app/app.qi", expected);

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "emit-interface", "--changed-only"])
        .arg(&project_root);
    let output = run_command_capture(&mut command, "`ql project emit-interface --changed-only`");
    let (stdout, stderr) = expect_success(
        "project-interface-changed-only-package",
        "changed-only package interface emission",
        &output,
    )
    .expect("changed-only package interface emission should succeed");
    expect_snapshot_matches(
        "project-interface-changed-only-package",
        "changed-only package interface stdout",
        &format!("up-to-date interface: {}\n", interface_path.display()),
        &stdout,
    )
    .expect("changed-only package interface emission should skip up-to-date artifact");
    expect_snapshot_matches(
        "project-interface-changed-only-package",
        "changed-only package interface stderr",
        "",
        &stderr,
    )
    .expect("changed-only package interface emission should stay silent on stderr");
    let actual = read_normalized_file(&interface_path, "changed-only generated qi artifact");
    expect_snapshot_matches(
        "project-interface-changed-only-package",
        "changed-only package qi artifact",
        expected,
        &actual,
    )
    .expect("changed-only package interface emission should leave up-to-date artifact unchanged");
}

#[test]
fn project_emit_interface_check_accepts_valid_package_interface() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-interface-check-package");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create project source directory for interface check test");
    let interface_path = project_root.join("app.qi");
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

pub fn exported() -> Int {
    return 1
}
"#,
    );
    temp.write(
        "workspace/app/app.qi",
        "\
// qlang interface v1
// package: app

// source: src/lib.ql
package demo.app

pub fn exported() -> Int
",
    );

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "emit-interface", "--check"])
        .arg(&project_root);
    let output = run_command_capture(&mut command, "`ql project emit-interface --check` package");
    let (stdout, stderr) = expect_success(
        "project-interface-check-package",
        "package interface check",
        &output,
    )
    .expect("package interface check should succeed");
    expect_snapshot_matches(
        "project-interface-check-package",
        "package interface check stdout",
        &format!("ok interface: {}\n", interface_path.display()),
        &stdout,
    )
    .expect("package interface check should report a valid interface");
    expect_snapshot_matches(
        "project-interface-check-package",
        "package interface check stderr",
        "",
        &stderr,
    )
    .expect("package interface check should stay silent on stderr");
}

#[test]
fn project_emit_interface_check_changed_only_accepts_valid_package_interface() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-interface-check-changed-only-package");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create project source directory for changed-only interface check test");
    let interface_path = project_root.join("app.qi");
    let expected = "\
// qlang interface v1
// package: app

// source: src/lib.ql
package demo.app

pub fn exported() -> Int
";
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

pub fn exported() -> Int {
    return 1
}
"#,
    );
    temp.write("workspace/app/app.qi", expected);
    let metadata_before = std::fs::metadata(&interface_path)
        .expect("read interface metadata before changed-only package check")
        .modified()
        .expect("read interface modification time before changed-only package check");

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "emit-interface", "--changed-only", "--check"])
        .arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql project emit-interface --changed-only --check` package",
    );
    let (stdout, stderr) = expect_success(
        "project-interface-check-changed-only-package",
        "changed-only package interface check",
        &output,
    )
    .expect("changed-only package interface check should succeed");
    expect_snapshot_matches(
        "project-interface-check-changed-only-package",
        "changed-only package interface check stdout",
        &format!("up-to-date interface: {}\n", interface_path.display()),
        &stdout,
    )
    .expect("changed-only package interface check should report a valid interface as up to date");
    expect_snapshot_matches(
        "project-interface-check-changed-only-package",
        "changed-only package interface check stderr",
        "",
        &stderr,
    )
    .expect("changed-only package interface check should stay silent on stderr");
    let actual = read_normalized_file(
        &interface_path,
        "changed-only package check interface artifact after check",
    );
    expect_snapshot_matches(
        "project-interface-check-changed-only-package",
        "changed-only package check qi artifact",
        expected,
        &actual,
    )
    .expect("changed-only package interface check should not rewrite a valid artifact");
    let metadata_after = std::fs::metadata(&interface_path)
        .expect("read interface metadata after changed-only package check")
        .modified()
        .expect("read interface modification time after changed-only package check");
    assert_eq!(
        metadata_before,
        metadata_after,
        "expected changed-only package interface check not to rewrite `{}`",
        interface_path.display()
    );
}

#[test]
fn project_emit_interface_check_rejects_invalid_package_interface() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-interface-check-invalid-package");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create project source directory for invalid interface check test");
    let manifest_path = temp.write(
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

pub fn exported() -> Int {
    return 1
}
"#,
    );
    temp.write("workspace/app/app.qi", "broken interface\n");

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "emit-interface", "--check"])
        .arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql project emit-interface --check` invalid package",
    );
    let (_stdout, stderr) = expect_exit_code(
        "project-interface-check-invalid-package",
        "invalid package interface check",
        &output,
        1,
    )
    .expect("invalid package interface check should fail");
    expect_stderr_contains(
        "project-interface-check-invalid-package",
        "invalid package interface check",
        &stderr,
        "is invalid",
    )
    .expect("invalid package interface check should report invalid status");
    expect_stderr_contains(
        "project-interface-check-invalid-package",
        "invalid package interface check",
        &stderr,
        "detail: expected `// qlang interface v1` header",
    )
    .expect("invalid package interface check should report parse detail");
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stderr_contains(
        "project-interface-check-invalid-package",
        "invalid package interface check",
        &normalized_stderr,
        &format!(
            "note: failing package manifest: {}",
            manifest_path.display().to_string().replace('\\', "/")
        ),
    )
    .expect("invalid package interface check should point to the failing package manifest");
    let error_line = format!(
        "error: interface artifact `{}` is invalid",
        project_root
            .join("app.qi")
            .display()
            .to_string()
            .replace('\\', "/")
    );
    let detail_line = "detail: expected `// qlang interface v1` header";
    let package_note = format!(
        "note: failing package manifest: {}",
        manifest_path.display().to_string().replace('\\', "/")
    );
    let rerun_hint = format!(
        "hint: rerun `ql project emit-interface {}` to regenerate it",
        manifest_path.display().to_string().replace('\\', "/")
    );
    let error_index = normalized_stderr
        .find(&error_line)
        .expect("invalid package interface check should report the error line");
    let detail_index = normalized_stderr
        .find(detail_line)
        .expect("invalid package interface check should report parse detail");
    let package_note_index = normalized_stderr
        .find(&package_note)
        .expect("invalid package interface check should point to the package manifest");
    let rerun_hint_index = normalized_stderr
        .find(&rerun_hint)
        .expect("invalid package interface check should include the rerun hint");
    assert!(
        error_index < detail_index
            && detail_index < package_note_index
            && package_note_index < rerun_hint_index,
        "expected invalid package interface check to keep detail before manifest and hint, got:\n{stderr}"
    );
}

#[test]
fn project_emit_interface_writes_member_qi_for_workspace_only_manifest() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-interface-workspace-only");
    let project_root = temp.path().join("workspace-only");
    let app_root = project_root.join("packages").join("app");
    let tool_root = project_root.join("packages").join("tool");
    std::fs::create_dir_all(app_root.join("src")).expect("create app package source directory");
    std::fs::create_dir_all(tool_root.join("src")).expect("create tool package source directory");
    temp.write(
        "workspace-only/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/tool"]
"#,
    );
    temp.write(
        "workspace-only/packages/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace-only/packages/app/src/lib.ql",
        r#"
package demo.app

pub fn exported() -> Int {
    return 1
}
"#,
    );
    temp.write(
        "workspace-only/packages/tool/qlang.toml",
        r#"
[package]
name = "tool"
"#,
    );
    temp.write(
        "workspace-only/packages/tool/src/lib.ql",
        r#"
package demo.tool

pub struct Config {
    value: Int,
}
"#,
    );
    let app_interface = app_root.join("app.qi");
    let tool_interface = tool_root.join("tool.qi");

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "emit-interface"])
        .arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql project emit-interface` workspace-only manifest",
    );
    let (stdout, stderr) = expect_success(
        "project-interface-workspace-only",
        "workspace-only interface emission",
        &output,
    )
    .expect("workspace-only interface emission should succeed");
    let normalized_stdout = stdout.replace('\\', "/");
    let normalized_app_interface = app_interface.display().to_string().replace('\\', "/");
    let normalized_tool_interface = tool_interface.display().to_string().replace('\\', "/");
    expect_stdout_contains_all(
        "project-interface-workspace-only",
        &normalized_stdout,
        &[
            &format!("wrote interface: {normalized_app_interface}"),
            &format!("wrote interface: {normalized_tool_interface}"),
        ],
    )
    .expect("workspace-only interface emission should report each written artifact");
    expect_snapshot_matches(
        "project-interface-workspace-only",
        "workspace-only interface emission stderr",
        &stderr,
        "",
    )
    .expect("workspace-only interface emission should stay silent on stderr");
    expect_file_exists(
        "project-interface-workspace-only",
        &app_interface,
        "workspace app qi",
        "workspace-only interface emission",
    )
    .expect("workspace-only interface emission should create app qi");
    expect_file_exists(
        "project-interface-workspace-only",
        &tool_interface,
        "workspace tool qi",
        "workspace-only interface emission",
    )
    .expect("workspace-only interface emission should create tool qi");
}

#[test]
fn project_emit_interface_keeps_writing_other_workspace_members_when_one_member_fails() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-interface-workspace-partial-failure");
    let project_root = temp.path().join("workspace-only");
    let app_root = project_root.join("packages").join("app");
    let broken_root = project_root.join("packages").join("broken");
    std::fs::create_dir_all(app_root.join("src"))
        .expect("create app package source directory for partial workspace emit test");
    std::fs::create_dir_all(&broken_root)
        .expect("create broken package directory for partial workspace emit test");
    temp.write(
        "workspace-only/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/broken"]
"#,
    );
    temp.write(
        "workspace-only/packages/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace-only/packages/app/src/lib.ql",
        r#"
package demo.app

pub fn exported() -> Int {
    return 1
}
"#,
    );
    temp.write(
        "workspace-only/packages/broken/qlang.toml",
        r#"
[package
name = "broken"
"#,
    );
    let app_interface = app_root.join("app.qi");
    let broken_interface = broken_root.join("broken.qi");

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "emit-interface"])
        .arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql project emit-interface` workspace manifest with failing member",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-interface-workspace-partial-failure",
        "workspace interface emission with failing member",
        &output,
        1,
    )
    .expect("workspace interface emission with failing member should fail");
    let normalized_stdout = stdout.replace('\\', "/");
    let normalized_stderr = stderr.replace('\\', "/");
    let normalized_app_interface = app_interface.display().to_string().replace('\\', "/");
    expect_stdout_contains_all(
        "project-interface-workspace-partial-failure",
        &normalized_stdout,
        &[&format!("wrote interface: {normalized_app_interface}")],
    )
    .expect("workspace interface emission should still write healthy members before failing");
    expect_stderr_contains(
        "project-interface-workspace-partial-failure",
        "workspace interface emission with failing member",
        &stderr,
        "invalid manifest",
    )
    .expect("workspace interface emission should surface the failing member manifest error");
    expect_stderr_contains(
        "project-interface-workspace-partial-failure",
        "workspace interface emission with failing member",
        &normalized_stderr,
        &format!(
            "note: failing workspace member manifest: {}",
            broken_root
                .join("qlang.toml")
                .display()
                .to_string()
                .replace('\\', "/")
        ),
    )
    .expect("workspace interface emission should point invalid member manifests locally");
    expect_stderr_contains(
        "project-interface-workspace-partial-failure",
        "workspace interface emission with failing member",
        &stderr,
        "interface emission found 1 failing member(s)",
    )
    .expect("workspace interface emission should summarize failing members");
    expect_stderr_not_contains(
        "project-interface-workspace-partial-failure",
        "workspace interface emission with failing member",
        &normalized_stderr,
        "note: first failing member manifest:",
    )
    .expect("single failing workspace members should not repeat the manifest in the final summary");
    expect_file_exists(
        "project-interface-workspace-partial-failure",
        &app_interface,
        "workspace app qi",
        "workspace interface emission with failing member",
    )
    .expect("workspace interface emission should still create app qi");
    assert!(
        !broken_interface.is_file(),
        "expected failing workspace member not to create `{}`",
        broken_interface.display()
    );
}

#[test]
fn project_emit_interface_points_workspace_member_source_failures_at_manifest() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-interface-workspace-source-failure");
    let project_root = temp.path().join("workspace-only");
    let app_root = project_root.join("packages").join("app");
    let broken_root = project_root.join("packages").join("broken");
    std::fs::create_dir_all(app_root.join("src"))
        .expect("create app package source directory for workspace source failure test");
    std::fs::create_dir_all(broken_root.join("src"))
        .expect("create broken package source directory for workspace source failure test");
    temp.write(
        "workspace-only/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/broken"]
"#,
    );
    temp.write(
        "workspace-only/packages/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace-only/packages/app/src/lib.ql",
        r#"
package demo.app

pub fn exported() -> Int {
    return 1
}
"#,
    );
    temp.write(
        "workspace-only/packages/broken/qlang.toml",
        r#"
[package]
name = "broken"
"#,
    );
    let first_failure = temp.write(
        "workspace-only/packages/broken/src/a_broken.ql",
        r#"
package demo.broken

pub fn broken_first(value: MissingFirst) -> Int {
    return value
}
"#,
    );
    temp.write(
        "workspace-only/packages/broken/src/z_broken.ql",
        r#"
package demo.broken

pub fn broken_second(value: MissingSecond) -> Int {
    return value
}
"#,
    );
    let app_interface = app_root.join("app.qi");
    let broken_manifest = broken_root.join("qlang.toml");
    let normalized_app_interface = app_interface.display().to_string().replace('\\', "/");
    let normalized_broken_manifest = broken_manifest.display().to_string().replace('\\', "/");
    let normalized_first_failure = first_failure.display().to_string().replace('\\', "/");

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "emit-interface"])
        .arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql project emit-interface` workspace member source failure",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-interface-workspace-source-failure",
        "workspace interface emission with member source failure",
        &output,
        1,
    )
    .expect("workspace interface emission with member source failure should fail");
    let normalized_stdout = stdout.replace('\\', "/");
    expect_stdout_contains_all(
        "project-interface-workspace-source-failure",
        &normalized_stdout,
        &[&format!("wrote interface: {normalized_app_interface}")],
    )
    .expect("workspace interface emission should still write healthy members");
    expect_stderr_contains(
        "project-interface-workspace-source-failure",
        "workspace interface emission with member source failure",
        &stderr,
        "a_broken.ql",
    )
    .expect("workspace interface emission should surface the first failing member source");
    expect_stderr_contains(
        "project-interface-workspace-source-failure",
        "workspace interface emission with member source failure",
        &stderr,
        "z_broken.ql",
    )
    .expect("workspace interface emission should continue surfacing later member source failures");
    expect_stderr_contains(
        "project-interface-workspace-source-failure",
        "workspace interface emission with member source failure",
        &stderr,
        "interface emission found 2 failing source file(s)",
    )
    .expect("workspace interface emission should preserve member source aggregation");
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stderr_contains(
        "project-interface-workspace-source-failure",
        "workspace interface emission with member source failure",
        &normalized_stderr,
        &format!("note: first failing source file: {normalized_first_failure}"),
    )
    .expect("workspace interface emission should point to the first failing member source file");
    expect_stderr_contains(
        "project-interface-workspace-source-failure",
        "workspace interface emission with member source failure",
        &normalized_stderr,
        &format!("note: failing package manifest: {normalized_broken_manifest}"),
    )
    .expect(
        "workspace interface emission should point member source failures at the member manifest",
    );
    expect_stderr_contains(
        "project-interface-workspace-source-failure",
        "workspace interface emission with member source failure",
        &normalized_stderr,
        &format!("note: failing workspace member manifest: {normalized_broken_manifest}"),
    )
    .expect("workspace interface emission should also keep the workspace member boundary visible");
    expect_stderr_contains(
        "project-interface-workspace-source-failure",
        "workspace interface emission with member source failure",
        &normalized_stderr,
        &format!(
            "hint: rerun `ql project emit-interface {}` after fixing the package interface error",
            normalized_broken_manifest
        ),
    )
    .expect("workspace interface emission should suggest rerunning the failing member manifest directly");
    let package_note = format!("note: failing package manifest: {normalized_broken_manifest}");
    let member_note =
        format!("note: failing workspace member manifest: {normalized_broken_manifest}");
    let rerun_hint = format!(
        "hint: rerun `ql project emit-interface {}` after fixing the package interface error",
        normalized_broken_manifest
    );
    let package_note_index = normalized_stderr
        .find(&package_note)
        .expect("workspace source failure should include the package manifest note");
    let member_note_index = normalized_stderr
        .find(&member_note)
        .expect("workspace source failure should include the workspace member note");
    let rerun_hint_index = normalized_stderr
        .find(&rerun_hint)
        .expect("workspace source failure should include the rerun hint");
    assert!(
        package_note_index < member_note_index && member_note_index < rerun_hint_index,
        "expected workspace source failure context before hint, got:\n{stderr}"
    );
    expect_stderr_contains(
        "project-interface-workspace-source-failure",
        "workspace interface emission with member source failure",
        &stderr,
        "interface emission found 1 failing member(s)",
    )
    .expect("workspace interface emission should still summarize failing members");
    expect_stderr_not_contains(
        "project-interface-workspace-source-failure",
        "workspace interface emission with member source failure",
        &normalized_stderr,
        "note: first failing member manifest:",
    )
    .expect("single failing workspace members should not repeat the manifest in the final summary");
}

#[test]
fn project_emit_interface_changed_only_rewrites_only_stale_workspace_members() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-interface-changed-only-workspace");
    let project_root = temp.path().join("workspace-only");
    let app_root = project_root.join("packages").join("app");
    let tool_root = project_root.join("packages").join("tool");
    std::fs::create_dir_all(app_root.join("src"))
        .expect("create app package source directory for changed-only workspace test");
    std::fs::create_dir_all(tool_root.join("src"))
        .expect("create tool package source directory for changed-only workspace test");
    temp.write(
        "workspace-only/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/tool"]
"#,
    );
    temp.write(
        "workspace-only/packages/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace-only/packages/app/src/lib.ql",
        r#"
package demo.app

pub fn exported() -> Int {
    return 1
}
"#,
    );
    temp.write(
        "workspace-only/packages/app/app.qi",
        "\
// qlang interface v1
// package: app

// source: src/lib.ql
package demo.app

pub fn exported() -> Int
",
    );
    temp.write(
        "workspace-only/packages/tool/qlang.toml",
        r#"
[package]
name = "tool"
"#,
    );
    temp.write(
        "workspace-only/packages/tool/src/lib.ql",
        r#"
package demo.tool

pub fn exported() -> Int {
    return 1
}
"#,
    );
    temp.write(
        "workspace-only/packages/tool/tool.qi",
        "\
// qlang interface v1
// package: tool

// source: src/lib.ql
package demo.tool

pub fn exported() -> Int
",
    );
    std::thread::sleep(std::time::Duration::from_millis(1200));
    temp.write(
        "workspace-only/packages/tool/src/lib.ql",
        r#"
package demo.tool

pub fn exported() -> Int {
    return 1
}

pub fn newer() -> Int {
    return 2
}
"#,
    );
    let app_interface = app_root.join("app.qi");
    let tool_interface = tool_root.join("tool.qi");

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "emit-interface", "--changed-only"])
        .arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql project emit-interface --changed-only` workspace-only manifest",
    );
    let (stdout, stderr) = expect_success(
        "project-interface-changed-only-workspace",
        "changed-only workspace interface emission",
        &output,
    )
    .expect("changed-only workspace interface emission should succeed");
    let normalized_stdout = stdout.replace('\\', "/");
    let normalized_app_interface = app_interface.display().to_string().replace('\\', "/");
    let normalized_tool_interface = tool_interface.display().to_string().replace('\\', "/");
    expect_stdout_contains_all(
        "project-interface-changed-only-workspace",
        &normalized_stdout,
        &[
            &format!("up-to-date interface: {normalized_app_interface}"),
            &format!("wrote interface: {normalized_tool_interface}"),
        ],
    )
    .expect("changed-only workspace interface emission should skip valid member and rewrite stale member");
    expect_snapshot_matches(
        "project-interface-changed-only-workspace",
        "changed-only workspace interface emission stderr",
        "",
        &stderr,
    )
    .expect("changed-only workspace interface emission should stay silent on stderr");
    let tool_actual = read_normalized_file(&tool_interface, "changed-only workspace tool qi");
    assert!(
        tool_actual.contains("pub fn newer() -> Int"),
        "expected stale workspace member interface to be regenerated, got:\n{tool_actual}"
    );
}

#[test]
fn project_emit_interface_check_rejects_stale_workspace_member_interface() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-interface-check-workspace");
    let project_root = temp.path().join("workspace-only");
    let app_root = project_root.join("packages").join("app");
    let tool_root = project_root.join("packages").join("tool");
    let broken_root = project_root.join("packages").join("broken");
    std::fs::create_dir_all(app_root.join("src"))
        .expect("create app package source directory for workspace interface check test");
    std::fs::create_dir_all(tool_root.join("src"))
        .expect("create tool package source directory for workspace interface check test");
    std::fs::create_dir_all(broken_root.join("src"))
        .expect("create broken package source directory for workspace interface check test");
    temp.write(
        "workspace-only/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/tool", "packages/broken"]
"#,
    );
    temp.write(
        "workspace-only/packages/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace-only/packages/app/src/lib.ql",
        r#"
package demo.app

pub fn exported() -> Int {
    return 1
}
"#,
    );
    temp.write(
        "workspace-only/packages/app/app.qi",
        "\
// qlang interface v1
// package: app

// source: src/lib.ql
package demo.app

pub fn exported() -> Int
",
    );
    temp.write(
        "workspace-only/packages/tool/qlang.toml",
        r#"
[package]
name = "tool"
"#,
    );
    temp.write(
        "workspace-only/packages/tool/src/lib.ql",
        r#"
package demo.tool

pub fn exported() -> Int {
    return 1
}
"#,
    );
    temp.write(
        "workspace-only/packages/tool/tool.qi",
        "\
// qlang interface v1
// package: tool

// source: src/lib.ql
package demo.tool

pub fn exported() -> Int
",
    );
    temp.write(
        "workspace-only/packages/broken/qlang.toml",
        r#"
[package]
name = "broken"
"#,
    );
    temp.write(
        "workspace-only/packages/broken/src/lib.ql",
        r#"
package demo.broken

pub fn exported() -> Int {
    return 3
}
"#,
    );
    std::thread::sleep(std::time::Duration::from_millis(1200));
    temp.write(
        "workspace-only/packages/tool/qlang.toml",
        r#"
[package]
name = "tool"
"#,
    );

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "emit-interface", "--check"])
        .arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql project emit-interface --check` workspace-only manifest",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-interface-check-workspace",
        "workspace interface check with stale member",
        &output,
        1,
    )
    .expect("workspace interface check with stale member should fail");
    let normalized_stdout = stdout.replace('\\', "/");
    let normalized_stderr = stderr.replace('\\', "/");
    let normalized_app_interface = app_root
        .join("app.qi")
        .display()
        .to_string()
        .replace('\\', "/");
    expect_stdout_contains_all(
        "project-interface-check-workspace",
        &normalized_stdout,
        &[&format!("ok interface: {normalized_app_interface}")],
    )
    .expect("workspace interface check should still report valid members");
    expect_stderr_contains(
        "project-interface-check-workspace",
        "workspace interface check with stale member",
        &stderr,
        "is stale",
    )
    .expect("workspace interface check should surface stale member interface status");
    expect_stderr_contains(
        "project-interface-check-workspace",
        "workspace interface check with stale member",
        &stderr,
        "reason: manifest newer than artifact:",
    )
    .expect("workspace interface check should explain why the stale member interface is stale");
    expect_stderr_contains(
        "project-interface-check-workspace",
        "workspace interface check with stale member",
        &stderr,
        "is missing",
    )
    .expect("workspace interface check should also surface missing member interface status");
    expect_stderr_contains(
        "project-interface-check-workspace",
        "workspace interface check with stale member",
        &normalized_stderr,
        &format!(
            "note: failing package manifest: {}",
            tool_root
                .join("qlang.toml")
                .display()
                .to_string()
                .replace('\\', "/")
        ),
    )
    .expect("workspace interface check should point stale member failures at the package manifest");
    expect_stderr_contains(
        "project-interface-check-workspace",
        "workspace interface check with stale member",
        &normalized_stderr,
        &format!(
            "note: failing workspace member manifest: {}",
            tool_root
                .join("qlang.toml")
                .display()
                .to_string()
                .replace('\\', "/")
        ),
    )
    .expect("workspace interface check should also keep the stale member boundary visible");
    let package_note = format!(
        "note: failing package manifest: {}",
        tool_root
            .join("qlang.toml")
            .display()
            .to_string()
            .replace('\\', "/")
    );
    let member_note = format!(
        "note: failing workspace member manifest: {}",
        tool_root
            .join("qlang.toml")
            .display()
            .to_string()
            .replace('\\', "/")
    );
    let rerun_hint = format!(
        "hint: rerun `ql project emit-interface {}` to regenerate it",
        tool_root
            .join("qlang.toml")
            .display()
            .to_string()
            .replace('\\', "/")
    );
    let package_note_index = normalized_stderr
        .find(&package_note)
        .expect("workspace stale member failure should include the package manifest note");
    let member_note_index = normalized_stderr
        .find(&member_note)
        .expect("workspace stale member failure should include the workspace member note");
    let rerun_hint_index = normalized_stderr
        .find(&rerun_hint)
        .expect("workspace stale member failure should include the rerun hint");
    assert!(
        package_note_index < member_note_index && member_note_index < rerun_hint_index,
        "expected workspace stale member context before hint, got:\n{stderr}"
    );
    expect_stderr_contains(
        "project-interface-check-workspace",
        "workspace interface check with stale member",
        &stderr,
        "found 2 failing member(s)",
    )
    .expect("workspace interface check should summarize all failing members");
    expect_stderr_contains(
        "project-interface-check-workspace",
        "workspace interface check with stale member",
        &normalized_stderr,
        &format!(
            "note: first failing member manifest: {}",
            tool_root
                .join("qlang.toml")
                .display()
                .to_string()
                .replace('\\', "/")
        ),
    )
    .expect("workspace interface check should point to the first failing member manifest");
}

#[test]
fn project_emit_interface_check_keeps_checking_other_workspace_members_when_one_member_manifest_is_invalid()
 {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-interface-check-workspace-invalid-member");
    let project_root = temp.path().join("workspace-only");
    let app_root = project_root.join("packages").join("app");
    let broken_root = project_root.join("packages").join("broken");
    std::fs::create_dir_all(app_root.join("src"))
        .expect("create app package source directory for invalid member workspace check test");
    std::fs::create_dir_all(&broken_root)
        .expect("create broken package directory for invalid member workspace check test");
    temp.write(
        "workspace-only/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/broken"]
"#,
    );
    temp.write(
        "workspace-only/packages/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace-only/packages/app/src/lib.ql",
        r#"
package demo.app

pub fn exported() -> Int {
    return 1
}
"#,
    );
    temp.write(
        "workspace-only/packages/app/app.qi",
        "\
// qlang interface v1
// package: app

// source: src/lib.ql
package demo.app

pub fn exported() -> Int
",
    );
    temp.write(
        "workspace-only/packages/broken/qlang.toml",
        r#"
[package
name = "broken"
"#,
    );

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "emit-interface", "--check"])
        .arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql project emit-interface --check` workspace manifest with invalid member",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-interface-check-workspace-invalid-member",
        "workspace interface check with invalid member manifest",
        &output,
        1,
    )
    .expect("workspace interface check with invalid member manifest should fail");
    let normalized_stdout = stdout.replace('\\', "/");
    let normalized_stderr = stderr.replace('\\', "/");
    let normalized_app_interface = app_root
        .join("app.qi")
        .display()
        .to_string()
        .replace('\\', "/");
    expect_stdout_contains_all(
        "project-interface-check-workspace-invalid-member",
        &normalized_stdout,
        &[&format!("ok interface: {normalized_app_interface}")],
    )
    .expect("workspace interface check should still report healthy members before failing");
    expect_stderr_contains(
        "project-interface-check-workspace-invalid-member",
        "workspace interface check with invalid member manifest",
        &stderr,
        "invalid manifest",
    )
    .expect("workspace interface check should surface the invalid member manifest");
    expect_stderr_contains(
        "project-interface-check-workspace-invalid-member",
        "workspace interface check with invalid member manifest",
        &normalized_stderr,
        &format!(
            "note: failing workspace member manifest: {}",
            broken_root
                .join("qlang.toml")
                .display()
                .to_string()
                .replace('\\', "/")
        ),
    )
    .expect("workspace interface check should point invalid member manifests locally");
    expect_stderr_contains(
        "project-interface-check-workspace-invalid-member",
        "workspace interface check with invalid member manifest",
        &stderr,
        "found 1 failing member(s)",
    )
    .expect("workspace interface check should summarize all failing members");
    expect_stderr_not_contains(
        "project-interface-check-workspace-invalid-member",
        "workspace interface check with invalid member manifest",
        &normalized_stderr,
        "note: first failing member manifest:",
    )
    .expect("single failing workspace members should not repeat the manifest in the final summary");
}

#[test]
fn project_emit_interface_check_changed_only_skips_valid_workspace_members() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-interface-check-changed-only-workspace");
    let project_root = temp.path().join("workspace-only");
    let app_root = project_root.join("packages").join("app");
    let tool_root = project_root.join("packages").join("tool");
    let broken_root = project_root.join("packages").join("broken");
    std::fs::create_dir_all(app_root.join("src")).expect(
        "create app package source directory for changed-only workspace interface check test",
    );
    std::fs::create_dir_all(tool_root.join("src")).expect(
        "create tool package source directory for changed-only workspace interface check test",
    );
    std::fs::create_dir_all(broken_root.join("src")).expect(
        "create broken package source directory for changed-only workspace interface check test",
    );
    temp.write(
        "workspace-only/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/tool", "packages/broken"]
"#,
    );
    temp.write(
        "workspace-only/packages/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace-only/packages/app/src/lib.ql",
        r#"
package demo.app

pub fn exported() -> Int {
    return 1
}
"#,
    );
    temp.write(
        "workspace-only/packages/app/app.qi",
        "\
// qlang interface v1
// package: app

// source: src/lib.ql
package demo.app

pub fn exported() -> Int
",
    );
    temp.write(
        "workspace-only/packages/tool/qlang.toml",
        r#"
[package]
name = "tool"
"#,
    );
    temp.write(
        "workspace-only/packages/tool/src/lib.ql",
        r#"
package demo.tool

pub fn exported() -> Int {
    return 1
}
"#,
    );
    temp.write(
        "workspace-only/packages/tool/tool.qi",
        "\
// qlang interface v1
// package: tool

// source: src/lib.ql
package demo.tool

pub fn exported() -> Int
",
    );
    temp.write(
        "workspace-only/packages/broken/qlang.toml",
        r#"
[package]
name = "broken"
"#,
    );
    temp.write(
        "workspace-only/packages/broken/src/lib.ql",
        r#"
package demo.broken

pub fn exported() -> Int {
    return 3
}
"#,
    );
    let app_interface = app_root.join("app.qi");
    let app_metadata_before = std::fs::metadata(&app_interface)
        .expect("read app interface metadata before changed-only workspace check")
        .modified()
        .expect("read app interface modification time before changed-only workspace check");
    std::thread::sleep(std::time::Duration::from_millis(1200));
    temp.write(
        "workspace-only/packages/tool/qlang.toml",
        r#"
[package]
name = "tool"
"#,
    );

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "emit-interface", "--changed-only", "--check"])
        .arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql project emit-interface --changed-only --check` workspace-only manifest",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-interface-check-changed-only-workspace",
        "changed-only workspace interface check with stale member",
        &output,
        1,
    )
    .expect("changed-only workspace interface check with stale member should fail");
    let normalized_stdout = stdout.replace('\\', "/");
    let normalized_app_interface = app_interface.display().to_string().replace('\\', "/");
    expect_snapshot_matches(
        "project-interface-check-changed-only-workspace",
        "changed-only workspace interface check stdout",
        &format!("up-to-date interface: {normalized_app_interface}\n"),
        &normalized_stdout,
    )
    .expect("changed-only workspace interface check should report valid members as up to date");
    expect_stderr_contains(
        "project-interface-check-changed-only-workspace",
        "changed-only workspace interface check with stale member",
        &stderr,
        "is stale",
    )
    .expect("changed-only workspace interface check should surface stale member interface status");
    expect_stderr_contains(
        "project-interface-check-changed-only-workspace",
        "changed-only workspace interface check with stale member",
        &stderr,
        "reason: manifest newer than artifact:",
    )
    .expect(
        "changed-only workspace interface check should explain why the stale member interface is stale",
    );
    expect_stderr_contains(
        "project-interface-check-changed-only-workspace",
        "changed-only workspace interface check with stale member",
        &stderr,
        "is missing",
    )
    .expect("changed-only workspace interface check should also surface missing member interface status");
    expect_stderr_contains(
        "project-interface-check-changed-only-workspace",
        "changed-only workspace interface check with stale member",
        &stderr,
        "found 2 failing member(s)",
    )
    .expect("changed-only workspace interface check should summarize all failing members");
    let app_metadata_after = std::fs::metadata(&app_interface)
        .expect("read app interface metadata after changed-only workspace check")
        .modified()
        .expect("read app interface modification time after changed-only workspace check");
    assert_eq!(
        app_metadata_before,
        app_metadata_after,
        "expected changed-only workspace interface check not to rewrite `{}`",
        app_interface.display()
    );
}

#[test]
fn project_emit_interface_rejects_output_path_for_workspace_only_manifest() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-interface-workspace-output");
    let project_root = temp.path().join("workspace-only");
    std::fs::create_dir_all(project_root.join("packages").join("app").join("src"))
        .expect("create workspace-only package directory");
    temp.write(
        "workspace-only/qlang.toml",
        r#"
[workspace]
members = ["packages/app"]
"#,
    );
    temp.write(
        "workspace-only/packages/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace-only/packages/app/src/lib.ql",
        r#"
package demo.app

pub fn exported() -> Int {
    return 1
}
"#,
    );

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "emit-interface"])
        .arg(&project_root)
        .args(["--output", "workspace.qi"]);
    let output = run_command_capture(
        &mut command,
        "`ql project emit-interface --output` workspace-only manifest",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-interface-workspace-output",
        "workspace-only interface emission with output",
        &output,
        1,
    )
    .expect("workspace-only interface emission with output should fail");
    expect_empty_stdout(
        "project-interface-workspace-output",
        "workspace-only interface emission with output",
        &stdout,
    )
    .expect("workspace-only interface emission with output should not print stdout");
    expect_stderr_contains(
        "project-interface-workspace-output",
        "workspace-only interface emission with output",
        &stderr,
        "--output` only supports package manifests",
    )
    .expect("workspace-only output rejection should explain the package-only constraint");
}

#[test]
fn build_with_emit_interface_writes_default_package_qi() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-build-emit-interface");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create project source directory for build interface test");
    let source_path = temp.write(
        "workspace/app/src/lib.ql",
        r#"
package demo.app

pub struct Buffer {
    value: Int,
}

pub fn exported(value: Int) -> Int {
    return value
}

fn main() -> Int {
    return exported(1)
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    let output_path = project_root.join("build").join("app.ll");
    let interface_path = project_root.join("app.qi");

    let mut command = ql_command(&workspace_root);
    command
        .arg("build")
        .arg(&source_path)
        .args(["--emit", "llvm-ir", "--output"])
        .arg(&output_path)
        .arg("--emit-interface");
    let output = run_command_capture(&mut command, "`ql build --emit-interface`");
    let (stdout, stderr) = expect_success(
        "build-emit-interface-success",
        "build with interface emission",
        &output,
    )
    .expect("build with interface emission should succeed");
    expect_stdout_contains_all(
        "build-emit-interface-success",
        &stdout,
        &[
            &format!("wrote llvm-ir: {}", output_path.display()),
            "wrote interface:",
            "app.qi",
        ],
    )
    .expect("build with interface emission should report both output artifacts");
    expect_snapshot_matches(
        "build-emit-interface-success",
        "build with interface emission stderr",
        "",
        &stderr,
    )
    .expect("successful build with interface emission should stay silent on stderr");
    expect_file_exists(
        "build-emit-interface-success",
        &output_path,
        "generated llvm ir",
        "build with interface emission",
    )
    .expect("build with interface emission should create the requested build artifact");
    expect_file_exists(
        "build-emit-interface-success",
        &interface_path,
        "generated interface",
        "build with interface emission",
    )
    .expect("build with interface emission should create the default package qi artifact");

    let expected = "\
// qlang interface v1
// package: app

// source: src/lib.ql
package demo.app

pub struct Buffer {
    value: Int,
}

pub fn exported(value: Int) -> Int
";
    let actual = read_normalized_file(&interface_path, "generated qi artifact");
    expect_snapshot_matches(
        "build-emit-interface-success",
        "generated qi artifact",
        expected,
        &actual,
    )
    .expect("generated qi artifact should match the build-side public interface snapshot");
}

#[test]
fn build_with_emit_interface_points_to_failing_package_manifest() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-build-emit-interface-failure");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create project source directory for build-side interface failure test");
    let source_path = temp.write(
        "workspace/app/src/lib.ql",
        r#"
package demo.app

pub fn exported(value: Int) -> Int {
    return value
}

fn main() -> Int {
    return exported(1)
}
"#,
    );
    let first_failure = temp.write(
        "workspace/app/src/a_broken.ql",
        r#"
package demo.app

pub fn broken_first(value: MissingFirst) -> Int {
    return value
}
"#,
    );
    temp.write(
        "workspace/app/src/z_broken.ql",
        r#"
package demo.app

pub fn broken_second(value: MissingSecond) -> Int {
    return value
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    let output_path = project_root.join("build").join("app.ll");
    let manifest_path = project_root.join("qlang.toml");
    let interface_path = project_root.join("app.qi");
    let manifest_display = manifest_path.to_string_lossy().replace('\\', "/");
    let output_display = output_path.to_string_lossy().replace('\\', "/");
    let first_failure_display = first_failure.to_string_lossy().replace('\\', "/");

    let mut command = ql_command(&workspace_root);
    command
        .arg("build")
        .arg(&source_path)
        .args(["--emit", "llvm-ir", "--output"])
        .arg(&output_path)
        .arg("--emit-interface");
    let output = run_command_capture(
        &mut command,
        "`ql build --emit-interface` with failing package interface emission",
    );
    let (stdout, stderr) = expect_exit_code(
        "build-emit-interface-failure",
        "build with failing interface emission",
        &output,
        1,
    )
    .expect("build should fail when package interface emission fails");
    expect_stdout_contains_all(
        "build-emit-interface-failure",
        &stdout,
        &[&format!("wrote llvm-ir: {}", output_path.display())],
    )
    .expect("build-side interface failure should still report the successful build artifact");
    expect_stderr_contains(
        "build-emit-interface-failure",
        "build with failing interface emission",
        &stderr,
        "a_broken.ql",
    )
    .expect("build-side interface failure should still surface the failing package source");
    expect_stderr_contains(
        "build-emit-interface-failure",
        "build with failing interface emission",
        &stderr,
        "z_broken.ql",
    )
    .expect("build-side interface failure should continue surfacing later package source failures");
    expect_stderr_contains(
        "build-emit-interface-failure",
        "build with failing interface emission",
        &stderr,
        "interface emission found 2 failing source file(s)",
    )
    .expect("build-side interface failure should summarize all failing package sources");
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stderr_contains(
        "build-emit-interface-failure",
        "build with failing interface emission",
        &normalized_stderr,
        &format!("note: first failing source file: {first_failure_display}"),
    )
    .expect("build-side interface failure should point to the first failing package source");
    expect_stderr_contains(
        "build-emit-interface-failure",
        "build with failing interface emission",
        &normalized_stderr,
        &format!("note: failing package manifest: {}", manifest_display),
    )
    .expect("build-side interface failure should point to the failing package manifest");
    expect_stderr_contains(
        "build-emit-interface-failure",
        "build with failing interface emission",
        &normalized_stderr,
        &format!(
            "hint: rerun `ql project emit-interface {}` after fixing the package interface error",
            manifest_display
        ),
    )
    .expect("build-side interface failure should suggest rerunning package interface emission");
    expect_stderr_contains(
        "build-emit-interface-failure",
        "build with failing interface emission",
        &normalized_stderr,
        &format!("note: build artifact remains at `{}`", output_display),
    )
    .expect("build-side interface failure should confirm that the build artifact was preserved");
    expect_file_exists(
        "build-emit-interface-failure",
        &output_path,
        "generated llvm ir",
        "build with failing interface emission",
    )
    .expect("build-side interface failure should keep the successful build artifact");
    assert!(
        !interface_path.is_file(),
        "build-side interface failure should not leave behind a partial interface artifact"
    );
}

#[test]
fn build_with_emit_interface_dedupes_single_failing_source_summary() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-build-emit-interface-single-failure");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create project source directory for single build-side interface failure test");
    let source_path = temp.write(
        "workspace/app/src/lib.ql",
        r#"
package demo.app

pub fn exported(value: Int) -> Int {
    return value
}

fn main() -> Int {
    return exported(1)
}
"#,
    );
    let broken_source = temp.write(
        "workspace/app/src/broken.ql",
        r#"
package demo.app

pub fn broken(value: MissingType) -> Int {
    return value
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    let output_path = project_root.join("build").join("app.ll");
    let manifest_path = project_root.join("qlang.toml");
    let interface_path = project_root.join("app.qi");
    let manifest_display = manifest_path.to_string_lossy().replace('\\', "/");
    let output_display = output_path.to_string_lossy().replace('\\', "/");
    let broken_source_display = broken_source.to_string_lossy().replace('\\', "/");

    let mut command = ql_command(&workspace_root);
    command
        .arg("build")
        .arg(&source_path)
        .args(["--emit", "llvm-ir", "--output"])
        .arg(&output_path)
        .arg("--emit-interface");
    let output = run_command_capture(
        &mut command,
        "`ql build --emit-interface` with single failing package interface source",
    );
    let (stdout, stderr) = expect_exit_code(
        "build-emit-interface-single-failure",
        "build with single failing interface source",
        &output,
        1,
    )
    .expect("build should fail when package interface emission has a single failing source");
    expect_stdout_contains_all(
        "build-emit-interface-single-failure",
        &stdout,
        &[&format!("wrote llvm-ir: {}", output_path.display())],
    )
    .expect("build-side single source failure should still report the successful build artifact");
    expect_stderr_contains(
        "build-emit-interface-single-failure",
        "build with single failing interface source",
        &stderr,
        "broken.ql",
    )
    .expect("build-side single source failure should surface the broken source file");
    expect_stderr_contains(
        "build-emit-interface-single-failure",
        "build with single failing interface source",
        &stderr,
        "interface emission found 1 failing source file(s)",
    )
    .expect("build-side single source failure should summarize the single failing source");
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stderr_not_contains(
        "build-emit-interface-single-failure",
        "build with single failing interface source",
        &normalized_stderr,
        "note: first failing source file:",
    )
    .expect(
        "single failing build-side sources should not repeat the source path in the final summary",
    );
    expect_stderr_contains(
        "build-emit-interface-single-failure",
        "build with single failing interface source",
        &normalized_stderr,
        &broken_source_display,
    )
    .expect("build-side single source failure should still surface the broken source path locally");
    expect_stderr_contains(
        "build-emit-interface-single-failure",
        "build with single failing interface source",
        &normalized_stderr,
        &format!("note: failing package manifest: {}", manifest_display),
    )
    .expect("build-side single source failure should still point to the failing package manifest");
    expect_stderr_contains(
        "build-emit-interface-single-failure",
        "build with single failing interface source",
        &normalized_stderr,
        &format!(
            "hint: rerun `ql project emit-interface {}` after fixing the package interface error",
            manifest_display
        ),
    )
    .expect("build-side single source failure should still suggest rerunning package interface emission");
    expect_stderr_contains(
        "build-emit-interface-single-failure",
        "build with single failing interface source",
        &normalized_stderr,
        &format!("note: build artifact remains at `{}`", output_display),
    )
    .expect(
        "build-side single source failure should confirm that the build artifact was preserved",
    );
    expect_file_exists(
        "build-emit-interface-single-failure",
        &output_path,
        "generated llvm ir",
        "build with single failing interface source",
    )
    .expect("build-side single source failure should keep the successful build artifact");
    assert!(
        !interface_path.is_file(),
        "build-side single source failure should not leave behind a partial interface artifact"
    );
}

#[test]
fn build_with_emit_interface_points_blocked_output_paths_at_interface_target() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-build-emit-interface-output-path-failure");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create project source directory for blocked build-side interface output test");
    let source_path = temp.write(
        "workspace/app/src/lib.ql",
        r#"
package demo.app

pub fn exported(value: Int) -> Int {
    return value
}

fn main() -> Int {
    return exported(1)
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    let interface_path = project_root.join("app.qi");
    std::fs::create_dir_all(&interface_path)
        .expect("create blocking interface directory for build-side emit-interface test");
    let output_path = project_root.join("build").join("app.ll");
    let manifest_path = project_root.join("qlang.toml");
    let manifest_display = manifest_path.to_string_lossy().replace('\\', "/");
    let interface_display = interface_path.to_string_lossy().replace('\\', "/");
    let output_display = output_path.to_string_lossy().replace('\\', "/");

    let mut command = ql_command(&workspace_root);
    command
        .arg("build")
        .arg(&source_path)
        .args(["--emit", "llvm-ir", "--output"])
        .arg(&output_path)
        .arg("--emit-interface");
    let output = run_command_capture(
        &mut command,
        "`ql build --emit-interface` with blocked interface output path",
    );
    let (stdout, stderr) = expect_exit_code(
        "build-emit-interface-output-path-failure",
        "build with blocked interface output path",
        &output,
        1,
    )
    .expect("build should fail when `--emit-interface` cannot write the default `.qi` path");
    expect_stdout_contains_all(
        "build-emit-interface-output-path-failure",
        &stdout,
        &[&format!("wrote llvm-ir: {}", output_path.display())],
    )
    .expect("build-side blocked output path should still report the successful build artifact");
    expect_stderr_contains(
        "build-emit-interface-output-path-failure",
        "build with blocked interface output path",
        &stderr,
        "failed to write interface",
    )
    .expect("build-side blocked output path should surface a write failure");
    let normalized_stderr = stderr.replace('\\', "/");
    let package_note = format!("note: failing package manifest: {manifest_display}");
    let output_note = format!("note: failing interface output path: {interface_display}");
    let rerun_hint = format!(
        "hint: rerun `ql project emit-interface {}` after fixing the interface output path",
        manifest_display
    );
    let old_hint = format!(
        "hint: rerun `ql project emit-interface {}` after fixing the package interface error",
        manifest_display
    );
    expect_stderr_contains(
        "build-emit-interface-output-path-failure",
        "build with blocked interface output path",
        &normalized_stderr,
        &package_note,
    )
    .expect("build-side blocked output path should still point to the package manifest");
    expect_stderr_contains(
        "build-emit-interface-output-path-failure",
        "build with blocked interface output path",
        &normalized_stderr,
        &output_note,
    )
    .expect("build-side blocked output path should point to the failing interface target");
    expect_stderr_contains(
        "build-emit-interface-output-path-failure",
        "build with blocked interface output path",
        &normalized_stderr,
        &rerun_hint,
    )
    .expect("build-side blocked output path should suggest fixing the interface output path");
    expect_stderr_not_contains(
        "build-emit-interface-output-path-failure",
        "build with blocked interface output path",
        &normalized_stderr,
        &old_hint,
    )
    .expect("build-side blocked output path should not reuse the package-source failure hint");
    expect_stderr_contains(
        "build-emit-interface-output-path-failure",
        "build with blocked interface output path",
        &normalized_stderr,
        &format!("note: build artifact remains at `{}`", output_display),
    )
    .expect("build-side blocked output path should confirm that the build artifact was preserved");
    expect_file_exists(
        "build-emit-interface-output-path-failure",
        &output_path,
        "generated llvm ir",
        "build with blocked interface output path",
    )
    .expect("build-side blocked output path should keep the successful build artifact");
    assert!(
        interface_path.is_dir(),
        "build-side blocked output path test should preserve `{}` as a directory",
        interface_path.display()
    );
}
