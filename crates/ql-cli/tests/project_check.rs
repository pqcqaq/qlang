mod support;

use support::{
    TempDir, expect_empty_stderr, expect_exit_code, expect_snapshot_matches,
    expect_stderr_contains, expect_stderr_not_contains, expect_stdout_contains_all, expect_success,
    ql_command, run_command_capture, workspace_root,
};

#[test]
fn check_package_dir_loads_referenced_interfaces() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check");
    let dep_root = temp.path().join("workspace").join("dep");
    let app_root = temp.path().join("workspace").join("app");
    let source_path = app_root.join("src").join("lib.ql");
    std::fs::create_dir_all(dep_root.join("src")).expect("create dependency source directory");
    std::fs::create_dir_all(app_root.join("src")).expect("create app source directory");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub const DEFAULT_PORT: Int
pub static BUILD_ID: Int

pub fn exported() -> Int

pub struct Buffer[T] {
    value: T,
}

impl Buffer[Int] {
    pub fn len(self) -> Int
}

extend Buffer[Int] {
    pub fn twice(self) -> Int
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
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
    let output = run_command_capture(&mut command, "`ql check` package dir");
    let (stdout, stderr) =
        expect_success("project-check-success", "package-aware ql check", &output)
            .expect("package-aware ql check should succeed");
    expect_stdout_contains_all(
        "project-check-success",
        &stdout,
        &[
            &format!("ok: {}", source_path.display()),
            "loaded interface: ",
            "dep.qi",
        ],
    )
    .expect("package-aware ql check should report sources and loaded interfaces");
    assert!(
        stderr.trim().is_empty(),
        "expected package-aware ql check stderr to stay empty, got:\n{stderr}"
    );
}

#[test]
fn check_package_dir_supports_json_output() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-json");
    let dep_root = temp.path().join("workspace").join("dep");
    let app_root = temp.path().join("workspace").join("app");
    let source_path = app_root.join("src").join("lib.ql");
    std::fs::create_dir_all(dep_root.join("src")).expect("create dependency source directory");
    std::fs::create_dir_all(app_root.join("src")).expect("create app source directory");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    let interface_path = temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub fn exported() -> Int
"#,
    );
    let manifest_path = temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
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
    command.args(["check", "--json"]).arg(&app_root);
    let output = run_command_capture(&mut command, "`ql check --json` package dir");
    let (stdout, stderr) = expect_success(
        "project-check-json-success",
        "package-aware ql check json",
        &output,
    )
    .expect("package-aware ql check json should succeed");
    expect_empty_stderr(
        "project-check-json-success",
        "package-aware ql check json",
        &stderr,
    )
    .expect("package-aware ql check json should not print stderr");

    let expected = format!(
        "{{\n  \"checked_files\": [\n    \"{}\"\n  ],\n  \"diagnostic_files\": [],\n  \"failing_manifests\": [],\n  \"loaded_interfaces\": [\n    \"{}\"\n  ],\n  \"project_manifest_path\": \"{}\",\n  \"schema\": \"ql.check.v1\",\n  \"scope\": \"package\",\n  \"status\": \"ok\",\n  \"sync_interfaces\": false,\n  \"written_interfaces\": []\n}}\n",
        source_path.display().to_string().replace('\\', "/"),
        interface_path.display().to_string().replace('\\', "/"),
        manifest_path.display().to_string().replace('\\', "/"),
    );
    expect_snapshot_matches(
        "project-check-json-success",
        "package check json stdout",
        &expected,
        &stdout.replace('\\', "/"),
    )
    .expect("package-aware ql check json should match the stable contract");
}

#[test]
fn check_source_file_loads_referenced_interfaces() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-source-file");
    let dep_root = temp.path().join("workspace").join("dep");
    let app_root = temp.path().join("workspace").join("app");
    let source_path = app_root.join("src").join("lib.ql");
    std::fs::create_dir_all(dep_root.join("src")).expect("create dependency source directory");
    std::fs::create_dir_all(app_root.join("src")).expect("create app source directory");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub fn exported() -> Int
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
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
    command.args(["check"]).arg(&source_path);
    let output = run_command_capture(&mut command, "`ql check` source file");
    let (stdout, stderr) = expect_success(
        "project-check-source-file-success",
        "package-aware ql check from source file",
        &output,
    )
    .expect("package-aware ql check from a source file should succeed");
    expect_stdout_contains_all(
        "project-check-source-file-success",
        &stdout,
        &[
            &format!("ok: {}", source_path.display()),
            "loaded interface: ",
            "dep.qi",
        ],
    )
    .expect("source-file package-aware ql check should report sources and loaded interfaces");
    assert!(
        stderr.trim().is_empty(),
        "expected package-aware ql check stderr to stay empty, got:\n{stderr}"
    );
}

#[test]
fn check_package_dir_reports_missing_dependency_interface() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-missing-interface");
    let dep_root = temp.path().join("workspace").join("dep");
    let app_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(dep_root.join("src")).expect("create dependency source directory");
    std::fs::create_dir_all(app_root.join("src")).expect("create app source directory");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    let app_manifest = temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
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
    let output = run_command_capture(&mut command, "`ql check` missing dependency interface");
    let (_stdout, stderr) = expect_exit_code(
        "project-check-missing-interface",
        "package-aware ql check with missing dependency interface",
        &output,
        1,
    )
    .expect("missing dependency interface should fail package-aware ql check");
    let error_line = format!(
        "error: `ql check` referenced package `dep` is missing interface artifact `{}`",
        dep_root
            .join("dep.qi")
            .display()
            .to_string()
            .replace('\\', "/")
    );
    let old_error_line = format!(
        "error: referenced package `dep` is missing interface artifact `{}`",
        dep_root
            .join("dep.qi")
            .display()
            .to_string()
            .replace('\\', "/")
    );
    expect_stderr_contains(
        "project-check-missing-interface",
        "package-aware ql check with missing dependency interface",
        &stderr,
        &error_line,
    )
    .expect("missing dependency interface should preserve the ql check command label");
    expect_stderr_not_contains(
        "project-check-missing-interface",
        "package-aware ql check with missing dependency interface",
        &stderr,
        &old_error_line,
    )
    .expect("missing dependency interface should not fall back to the unlabeled artifact error");
    expect_stderr_contains(
        "project-check-missing-interface",
        "package-aware ql check with missing dependency interface",
        &stderr,
        "--sync-interfaces",
    )
    .expect("missing dependency interface diagnostic should suggest sync");
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stderr_contains(
        "project-check-missing-interface",
        "package-aware ql check with missing dependency interface",
        &normalized_stderr,
        &format!(
            "note: failing referenced package manifest: {}",
            dep_root
                .join("qlang.toml")
                .display()
                .to_string()
                .replace('\\', "/")
        ),
    )
    .expect(
        "missing dependency interface diagnostic should point to the referenced package manifest",
    );
    expect_stderr_contains(
        "project-check-missing-interface",
        "package-aware ql check with missing dependency interface",
        &normalized_stderr,
        &format!(
            "note: while checking referenced package `../dep` from `{}`",
            app_manifest.display().to_string().replace('\\', "/")
        ),
    )
    .expect("missing dependency interface diagnostic should point back to the owner reference");
    let package_note = format!(
        "note: failing package manifest: {}",
        app_manifest.display().to_string().replace('\\', "/")
    );
    let rerun_hint = format!(
        "hint: rerun `ql check {}` after fixing the referenced package or reference manifest",
        app_manifest.display().to_string().replace('\\', "/")
    );
    expect_stderr_contains(
        "project-check-missing-interface",
        "package-aware ql check with missing dependency interface",
        &normalized_stderr,
        &package_note,
    )
    .expect("missing dependency interface diagnostic should point to the failing package manifest");
    expect_stderr_contains(
        "project-check-missing-interface",
        "package-aware ql check with missing dependency interface",
        &normalized_stderr,
        &rerun_hint,
    )
    .expect("missing dependency interface diagnostic should suggest rerunning the package manifest directly");
    let owner_note = format!(
        "note: while checking referenced package `../dep` from `{}`",
        app_manifest.display().to_string().replace('\\', "/")
    );
    let owner_note_index = normalized_stderr
        .find(&owner_note)
        .expect("missing dependency interface diagnostic should include the owner note");
    let package_note_index = normalized_stderr
        .find(&package_note)
        .expect("missing dependency interface diagnostic should include the package note");
    let rerun_hint_index = normalized_stderr
        .rfind(&rerun_hint)
        .expect("missing dependency interface diagnostic should include the direct rerun hint");
    assert!(
        owner_note_index < package_note_index && package_note_index < rerun_hint_index,
        "expected direct package reference context before direct rerun hint, got:\n{stderr}"
    );
    expect_stderr_not_contains(
        "project-check-missing-interface",
        "package-aware ql check with missing dependency interface",
        &normalized_stderr,
        "note: first failing reference manifest:",
    )
    .expect("single failing references should not repeat the manifest in the final summary");
}

#[test]
fn check_package_dir_reports_invalid_referenced_manifest() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-invalid-reference-manifest");
    let app_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(app_root.join("src")).expect("create app source directory");
    std::fs::create_dir_all(temp.path().join("workspace").join("workspace_ref"))
        .expect("create workspace-only reference directory");

    temp.write(
        "workspace/workspace_ref/qlang.toml",
        r#"
[workspace]
members = ["packages/demo"]
"#,
    );
    let app_manifest = temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../workspace_ref"]
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
    let output = run_command_capture(&mut command, "`ql check` invalid referenced manifest");
    let (_stdout, stderr) = expect_exit_code(
        "project-check-invalid-reference-manifest",
        "package-aware ql check with invalid referenced manifest",
        &output,
        1,
    )
    .expect("invalid referenced manifest should fail package-aware ql check");
    let error_line = "error: `ql check` failed to load referenced package `../workspace_ref`";
    let old_error_line = "error: failed to load referenced package `../workspace_ref`";
    expect_stderr_contains(
        "project-check-invalid-reference-manifest",
        "package-aware ql check with invalid referenced manifest",
        &stderr,
        error_line,
    )
    .expect("invalid referenced manifest should preserve the ql check command label");
    expect_stderr_not_contains(
        "project-check-invalid-reference-manifest",
        "package-aware ql check with invalid referenced manifest",
        &stderr,
        old_error_line,
    )
    .expect("invalid referenced manifest should not fall back to the unlabeled reference error");
    expect_stderr_contains(
        "project-check-invalid-reference-manifest",
        "package-aware ql check with invalid referenced manifest",
        &stderr,
        "does not declare `[package].name`",
    )
    .expect("invalid referenced manifest should surface the manifest detail");
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stderr_contains(
        "project-check-invalid-reference-manifest",
        "package-aware ql check with invalid referenced manifest",
        &normalized_stderr,
        &format!(
            "note: failing reference manifest: {}",
            temp.path()
                .join("workspace")
                .join("workspace_ref")
                .join("qlang.toml")
                .display()
                .to_string()
                .replace('\\', "/")
        ),
    )
    .expect("invalid referenced manifest should point to the broken manifest path");
    expect_stderr_contains(
        "project-check-invalid-reference-manifest",
        "package-aware ql check with invalid referenced manifest",
        &normalized_stderr,
        &format!(
            "fix the reference in `{}`",
            app_manifest.display().to_string().replace('\\', "/")
        ),
    )
    .expect("invalid referenced manifest should hint at the owning manifest");
}

#[test]
fn check_package_dir_reports_invalid_dependency_interface() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-invalid-interface");
    let dep_root = temp.path().join("workspace").join("dep");
    let app_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(dep_root.join("src")).expect("create dependency source directory");
    std::fs::create_dir_all(app_root.join("src")).expect("create app source directory");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "workspace/dep/dep.qi",
        r#"
not a valid interface
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
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
    let output = run_command_capture(&mut command, "`ql check` invalid dependency interface");
    let (_stdout, stderr) = expect_exit_code(
        "project-check-invalid-interface",
        "package-aware ql check with invalid dependency interface",
        &output,
        1,
    )
    .expect("invalid dependency interface should fail package-aware ql check");
    expect_stderr_contains(
        "project-check-invalid-interface",
        "package-aware ql check with invalid dependency interface",
        &stderr,
        "referenced package `dep` has invalid interface artifact",
    )
    .expect("invalid dependency interface should surface a clear error");
    expect_stderr_contains(
        "project-check-invalid-interface",
        "package-aware ql check with invalid dependency interface",
        &stderr,
        "detail:",
    )
    .expect("invalid dependency interface should surface parse detail");
    expect_stderr_contains(
        "project-check-invalid-interface",
        "package-aware ql check with invalid dependency interface",
        &stderr,
        "--sync-interfaces",
    )
    .expect("invalid dependency interface diagnostic should suggest sync");
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stderr_contains(
        "project-check-invalid-interface",
        "package-aware ql check with invalid dependency interface",
        &normalized_stderr,
        &format!(
            "note: failing referenced package manifest: {}",
            dep_root
                .join("qlang.toml")
                .display()
                .to_string()
                .replace('\\', "/")
        ),
    )
    .expect("invalid dependency interface should point to the referenced package manifest");
    let error_line = format!(
        "error: `ql check` referenced package `dep` has invalid interface artifact `{}`",
        dep_root
            .join("dep.qi")
            .display()
            .to_string()
            .replace('\\', "/")
    );
    let old_error_line = format!(
        "error: referenced package `dep` has invalid interface artifact `{}`",
        dep_root
            .join("dep.qi")
            .display()
            .to_string()
            .replace('\\', "/")
    );
    expect_stderr_not_contains(
        "project-check-invalid-interface",
        "package-aware ql check with invalid dependency interface",
        &normalized_stderr,
        &old_error_line,
    )
    .expect("invalid dependency interface should not fall back to the unlabeled artifact error");
    let detail_line = "detail: expected `// qlang interface v1` header";
    let failing_manifest_note = format!(
        "note: failing referenced package manifest: {}",
        dep_root
            .join("qlang.toml")
            .display()
            .to_string()
            .replace('\\', "/")
    );
    let owner_note = format!(
        "note: while checking referenced package `../dep` from `{}`",
        app_root
            .join("qlang.toml")
            .display()
            .to_string()
            .replace('\\', "/")
    );
    let rerun_hint = format!(
        "hint: rerun `ql check --sync-interfaces {}` or regenerate `dep` with `ql project emit-interface {}`",
        app_root
            .join("qlang.toml")
            .display()
            .to_string()
            .replace('\\', "/"),
        dep_root
            .join("qlang.toml")
            .display()
            .to_string()
            .replace('\\', "/")
    );
    let error_index = normalized_stderr
        .find(&error_line)
        .expect("invalid dependency interface should report the error line");
    let detail_index = normalized_stderr
        .find(detail_line)
        .expect("invalid dependency interface should report parse detail");
    let failing_manifest_index = normalized_stderr
        .find(&failing_manifest_note)
        .expect("invalid dependency interface should point to the referenced manifest");
    let owner_note_index = normalized_stderr
        .find(&owner_note)
        .expect("invalid dependency interface should point back to the owner manifest");
    let rerun_hint_index = normalized_stderr
        .find(&rerun_hint)
        .expect("invalid dependency interface should include the repair hint");
    assert!(
        error_index < detail_index
            && detail_index < failing_manifest_index
            && failing_manifest_index < owner_note_index
            && owner_note_index < rerun_hint_index,
        "expected invalid dependency interface diagnostic order error -> detail -> manifests -> hint, got:\n{stderr}"
    );
}

#[test]
fn check_package_dir_reports_all_failing_references() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-multiple-reference-failures");
    let dep_root = temp.path().join("workspace").join("dep");
    let app_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(dep_root.join("src")).expect("create dependency source directory");
    std::fs::create_dir_all(app_root.join("src")).expect("create app source directory");
    std::fs::create_dir_all(temp.path().join("workspace").join("workspace_ref"))
        .expect("create workspace-only reference directory");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "workspace/workspace_ref/qlang.toml",
        r#"
[workspace]
members = ["packages/demo"]
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep", "../workspace_ref"]
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
    let output = run_command_capture(&mut command, "`ql check` multiple reference failures");
    let (_stdout, stderr) = expect_exit_code(
        "project-check-multiple-reference-failures",
        "package-aware ql check with multiple failing references",
        &output,
        1,
    )
    .expect("multiple failing references should fail package-aware ql check");
    expect_stderr_contains(
        "project-check-multiple-reference-failures",
        "package-aware ql check with multiple failing references",
        &stderr,
        "referenced package `dep` is missing interface artifact",
    )
    .expect("package-aware ql check should still surface missing dependency interfaces");
    expect_stderr_contains(
        "project-check-multiple-reference-failures",
        "package-aware ql check with multiple failing references",
        &stderr,
        "failed to load referenced package `../workspace_ref`",
    )
    .expect("package-aware ql check should continue and surface later broken manifests");
    expect_stderr_contains(
        "project-check-multiple-reference-failures",
        "package-aware ql check with multiple failing references",
        &stderr,
        "`ql check` found 2 failing referenced package(s)",
    )
    .expect("package-aware ql check should summarize all failing references");
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stderr_contains(
        "project-check-multiple-reference-failures",
        "package-aware ql check with multiple failing references",
        &normalized_stderr,
        &format!(
            "note: first failing reference manifest: {}",
            dep_root
                .join("qlang.toml")
                .display()
                .to_string()
                .replace('\\', "/")
        ),
    )
    .expect("package-aware ql check should point to the first failing reference manifest");
}

#[test]
fn check_package_dir_reports_transitive_reference_failures_when_direct_interface_is_missing() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-transitive-reference-failures");
    let dep_root = temp.path().join("workspace").join("dep");
    let app_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(dep_root.join("src")).expect("create dependency source directory");
    std::fs::create_dir_all(app_root.join("src")).expect("create app source directory");
    std::fs::create_dir_all(temp.path().join("workspace").join("broken_ref"))
        .expect("create broken reference directory");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"

[references]
packages = ["../broken_ref"]
"#,
    );
    temp.write(
        "workspace/dep/src/lib.ql",
        r#"
package demo.dep

pub fn exported() -> Int {
    return 7
}
"#,
    );
    temp.write(
        "workspace/broken_ref/qlang.toml",
        r#"
[package
name = "broken_ref"
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
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
    let output = run_command_capture(
        &mut command,
        "`ql check` transitive reference failures with missing direct interface",
    );
    let (_stdout, stderr) = expect_exit_code(
        "project-check-transitive-reference-failures",
        "package-aware ql check with transitive reference failures",
        &output,
        1,
    )
    .expect("transitive reference failures should fail package-aware ql check");
    expect_stderr_contains(
        "project-check-transitive-reference-failures",
        "package-aware ql check with transitive reference failures",
        &stderr,
        "referenced package `dep` is missing interface artifact",
    )
    .expect("package-aware ql check should still surface the direct missing interface");
    expect_stderr_contains(
        "project-check-transitive-reference-failures",
        "package-aware ql check with transitive reference failures",
        &stderr,
        "failed to load referenced package `../broken_ref`",
    )
    .expect("package-aware ql check should continue into transitive broken references");
    expect_stderr_contains(
        "project-check-transitive-reference-failures",
        "package-aware ql check with transitive reference failures",
        &stderr,
        "`ql check` found 2 failing referenced package(s)",
    )
    .expect("package-aware ql check should summarize direct and transitive failures");
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stderr_contains(
        "project-check-transitive-reference-failures",
        "package-aware ql check with transitive reference failures",
        &normalized_stderr,
        &format!(
            "note: first failing reference manifest: {}",
            dep_root
                .join("qlang.toml")
                .display()
                .to_string()
                .replace('\\', "/")
        ),
    )
    .expect("package-aware ql check should point to the first failing direct manifest");
}

#[test]
fn check_package_dir_sync_interfaces_reports_invalid_referenced_manifest() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-sync-invalid-reference-manifest");
    let app_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(app_root.join("src")).expect("create app source directory");
    std::fs::create_dir_all(temp.path().join("workspace").join("broken_ref"))
        .expect("create broken reference directory");

    temp.write(
        "workspace/broken_ref/qlang.toml",
        r#"
[package
name = "broken_ref"
"#,
    );
    let app_manifest = temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../broken_ref"]
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
        "`ql check --sync-interfaces` invalid referenced manifest",
    );
    let (_stdout, stderr) = expect_exit_code(
        "project-check-sync-invalid-reference-manifest",
        "package-aware ql check sync with invalid referenced manifest",
        &output,
        1,
    )
    .expect("sync path should fail on invalid referenced manifest");
    let error_line =
        "error: `ql check --sync-interfaces` failed to load referenced package `../broken_ref`";
    let old_error_line = "error: failed to load referenced package `../broken_ref`";
    expect_stderr_contains(
        "project-check-sync-invalid-reference-manifest",
        "package-aware ql check sync with invalid referenced manifest",
        &stderr,
        error_line,
    )
    .expect("sync path should preserve the ql check sync command label");
    expect_stderr_not_contains(
        "project-check-sync-invalid-reference-manifest",
        "package-aware ql check sync with invalid referenced manifest",
        &stderr,
        old_error_line,
    )
    .expect("sync path should not fall back to the unlabeled reference error");
    expect_stderr_contains(
        "project-check-sync-invalid-reference-manifest",
        "package-aware ql check sync with invalid referenced manifest",
        &stderr,
        "invalid manifest `",
    )
    .expect("sync path should surface the manifest parse detail");
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stderr_contains(
        "project-check-sync-invalid-reference-manifest",
        "package-aware ql check sync with invalid referenced manifest",
        &normalized_stderr,
        &format!(
            "note: failing reference manifest: {}",
            temp.path()
                .join("workspace")
                .join("broken_ref")
                .join("qlang.toml")
                .display()
                .to_string()
                .replace('\\', "/")
        ),
    )
    .expect("sync path should point to the broken manifest path");
    expect_stderr_contains(
        "project-check-sync-invalid-reference-manifest",
        "package-aware ql check sync with invalid referenced manifest",
        &normalized_stderr,
        &format!(
            "fix the reference in `{}`",
            app_manifest.display().to_string().replace('\\', "/")
        ),
    )
    .expect("sync path should hint at the owning manifest");
    let package_note = format!(
        "note: failing package manifest: {}",
        app_manifest.display().to_string().replace('\\', "/")
    );
    let rerun_hint = format!(
        "hint: rerun `ql check --sync-interfaces {}` after fixing the referenced package or reference manifest",
        app_manifest.display().to_string().replace('\\', "/")
    );
    expect_stderr_contains(
        "project-check-sync-invalid-reference-manifest",
        "package-aware ql check sync with invalid referenced manifest",
        &normalized_stderr,
        &package_note,
    )
    .expect("sync path should point to the failing package manifest");
    expect_stderr_contains(
        "project-check-sync-invalid-reference-manifest",
        "package-aware ql check sync with invalid referenced manifest",
        &normalized_stderr,
        &rerun_hint,
    )
    .expect("sync path should suggest rerunning the package manifest directly");
    let reference_note = format!(
        "note: failing reference manifest: {}",
        temp.path()
            .join("workspace")
            .join("broken_ref")
            .join("qlang.toml")
            .display()
            .to_string()
            .replace('\\', "/")
    );
    let reference_note_index = normalized_stderr
        .find(&reference_note)
        .expect("sync path should include the failing reference note");
    let package_note_index = normalized_stderr
        .find(&package_note)
        .expect("sync path should include the package note");
    let rerun_hint_index = normalized_stderr
        .rfind(&rerun_hint)
        .expect("sync path should include the direct rerun hint");
    assert!(
        reference_note_index < package_note_index && package_note_index < rerun_hint_index,
        "expected direct package sync reference context before direct rerun hint, got:\n{stderr}"
    );
    expect_stderr_not_contains(
        "project-check-sync-invalid-reference-manifest",
        "package-aware ql check sync with invalid referenced manifest",
        &normalized_stderr,
        "note: first failing reference manifest:",
    )
    .expect("single failing references on the sync path should not repeat the manifest in the final summary");
}

#[test]
fn check_package_dir_sync_interfaces_reports_all_failing_references() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-sync-multiple-reference-failures");
    let dep_root = temp.path().join("workspace").join("dep");
    let app_root = temp.path().join("workspace").join("app");
    let interface_path = dep_root.join("dep.qi");
    std::fs::create_dir_all(dep_root.join("src")).expect("create dependency source directory");
    std::fs::create_dir_all(app_root.join("src")).expect("create app source directory");
    std::fs::create_dir_all(temp.path().join("workspace").join("broken_ref"))
        .expect("create broken reference directory");
    std::fs::create_dir_all(temp.path().join("workspace").join("workspace_ref"))
        .expect("create workspace-only reference directory");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "workspace/dep/src/lib.ql",
        r#"
package demo.dep

pub fn exported() -> Int {
    return 7
}
"#,
    );
    temp.write(
        "workspace/broken_ref/qlang.toml",
        r#"
[package
name = "broken_ref"
"#,
    );
    temp.write(
        "workspace/workspace_ref/qlang.toml",
        r#"
[workspace]
members = ["packages/demo"]
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep", "../broken_ref", "../workspace_ref"]
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
        "`ql check --sync-interfaces` multiple reference failures",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-check-sync-multiple-reference-failures",
        "package-aware ql check sync with multiple failing references",
        &output,
        1,
    )
    .expect("multiple failing references should fail package-aware sync");
    expect_stdout_contains_all(
        "project-check-sync-multiple-reference-failures",
        &stdout,
        &["wrote interface: ", "dep.qi"],
    )
    .expect("sync path should still emit interfaces for healthy references");
    assert!(
        interface_path.is_file(),
        "expected synced dependency interface at `{}`",
        interface_path.display()
    );
    expect_stderr_contains(
        "project-check-sync-multiple-reference-failures",
        "package-aware ql check sync with multiple failing references",
        &stderr,
        "failed to load referenced package `../broken_ref`",
    )
    .expect("sync path should surface broken manifests");
    expect_stderr_contains(
        "project-check-sync-multiple-reference-failures",
        "package-aware ql check sync with multiple failing references",
        &stderr,
        "failed to load referenced package `../workspace_ref`",
    )
    .expect("sync path should continue and surface later invalid package references");
    expect_stderr_contains(
        "project-check-sync-multiple-reference-failures",
        "package-aware ql check sync with multiple failing references",
        &stderr,
        "`ql check --sync-interfaces` found 2 failing referenced package(s)",
    )
    .expect("sync path should summarize all failing references");
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stderr_contains(
        "project-check-sync-multiple-reference-failures",
        "package-aware ql check sync with multiple failing references",
        &normalized_stderr,
        &format!(
            "note: first failing reference manifest: {}",
            temp.path()
                .join("workspace")
                .join("broken_ref")
                .join("qlang.toml")
                .display()
                .to_string()
                .replace('\\', "/")
        ),
    )
    .expect("sync path should point to the first failing reference manifest");
}

#[test]
fn check_package_dir_sync_interfaces_emits_direct_dependency_before_transitive_failure() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-sync-transitive-reference-failures");
    let dep_root = temp.path().join("workspace").join("dep");
    let app_root = temp.path().join("workspace").join("app");
    let interface_path = dep_root.join("dep.qi");
    std::fs::create_dir_all(dep_root.join("src")).expect("create dependency source directory");
    std::fs::create_dir_all(app_root.join("src")).expect("create app source directory");
    std::fs::create_dir_all(temp.path().join("workspace").join("broken_ref"))
        .expect("create broken reference directory");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"

[references]
packages = ["../broken_ref"]
"#,
    );
    temp.write(
        "workspace/dep/src/lib.ql",
        r#"
package demo.dep

pub fn exported() -> Int {
    return 7
}
"#,
    );
    temp.write(
        "workspace/broken_ref/qlang.toml",
        r#"
[package
name = "broken_ref"
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
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
        "`ql check --sync-interfaces` transitive reference failures",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-check-sync-transitive-reference-failures",
        "package-aware ql check sync with transitive reference failures",
        &output,
        1,
    )
    .expect("transitive reference failures should still fail package-aware sync");
    expect_stdout_contains_all(
        "project-check-sync-transitive-reference-failures",
        &stdout,
        &["wrote interface: ", "dep.qi"],
    )
    .expect("sync path should still emit the direct dependency interface");
    assert!(
        interface_path.is_file(),
        "expected synced dependency interface at `{}`",
        interface_path.display()
    );
    expect_stderr_contains(
        "project-check-sync-transitive-reference-failures",
        "package-aware ql check sync with transitive reference failures",
        &stderr,
        "failed to load referenced package `../broken_ref`",
    )
    .expect("sync path should continue into transitive broken references");
    expect_stderr_contains(
        "project-check-sync-transitive-reference-failures",
        "package-aware ql check sync with transitive reference failures",
        &stderr,
        "`ql check --sync-interfaces` found 1 failing referenced package(s)",
    )
    .expect("sync path should only summarize the remaining transitive failure");
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stderr_not_contains(
        "project-check-sync-transitive-reference-failures",
        "package-aware ql check sync with transitive reference failures",
        &normalized_stderr,
        "note: first failing reference manifest:",
    )
    .expect(
        "single remaining transitive failures should not repeat the manifest in the final summary",
    );
}

#[test]
fn check_package_dir_sync_interfaces_points_dependency_source_failures_at_owner_reference() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-sync-source-failure-context");
    let dep_root = temp.path().join("workspace").join("dep");
    let app_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(dep_root.join("src")).expect("create dependency source directory");
    std::fs::create_dir_all(app_root.join("src")).expect("create app source directory");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "workspace/dep/src/a_broken.ql",
        r#"
package demo.dep

pub fn broken_first(value: MissingFirst) -> Int {
    return value
}
"#,
    );
    temp.write(
        "workspace/dep/src/z_broken.ql",
        r#"
package demo.dep

pub fn broken_second(value: MissingSecond) -> Int {
    return value
}
"#,
    );
    let app_manifest = temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
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
        "`ql check --sync-interfaces` dependency source failure context",
    );
    let (_stdout, stderr) = expect_exit_code(
        "project-check-sync-source-failure-context",
        "package-aware ql check sync with dependency source failures",
        &output,
        1,
    )
    .expect("dependency source failures should fail package-aware sync");
    expect_stderr_contains(
        "project-check-sync-source-failure-context",
        "package-aware ql check sync with dependency source failures",
        &stderr,
        "a_broken.ql",
    )
    .expect("sync path should surface the first failing dependency source file");
    expect_stderr_contains(
        "project-check-sync-source-failure-context",
        "package-aware ql check sync with dependency source failures",
        &stderr,
        "z_broken.ql",
    )
    .expect("sync path should continue surfacing later dependency source failures");
    expect_stderr_contains(
        "project-check-sync-source-failure-context",
        "package-aware ql check sync with dependency source failures",
        &stderr,
        "`ql check --sync-interfaces` found 2 failing source file(s)",
    )
    .expect("sync path should preserve package-level source failure aggregation");
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stderr_contains(
        "project-check-sync-source-failure-context",
        "package-aware ql check sync with dependency source failures",
        &normalized_stderr,
        &format!(
            "note: first failing source file: {}",
            dep_root
                .join("src")
                .join("a_broken.ql")
                .display()
                .to_string()
                .replace('\\', "/")
        ),
    )
    .expect("sync path should point to the first failing dependency source file");
    expect_stderr_contains(
        "project-check-sync-source-failure-context",
        "package-aware ql check sync with dependency source failures",
        &normalized_stderr,
        &format!(
            "note: while syncing referenced package `../dep` from `{}`",
            app_manifest.display().to_string().replace('\\', "/")
        ),
    )
    .expect("sync path should point the dependency source failure back to the owner reference");
    expect_stderr_contains(
        "project-check-sync-source-failure-context",
        "package-aware ql check sync with dependency source failures",
        &normalized_stderr,
        &format!(
            "note: failing package manifest: {}",
            dep_root
                .join("qlang.toml")
                .display()
                .to_string()
                .replace('\\', "/")
        ),
    )
    .expect("sync path should point to the failing dependency manifest");
    expect_stderr_contains(
        "project-check-sync-source-failure-context",
        "package-aware ql check sync with dependency source failures",
        &normalized_stderr,
        &format!(
            "hint: rerun `ql project emit-interface {}` after fixing the package sources",
            dep_root
                .join("qlang.toml")
                .display()
                .to_string()
                .replace('\\', "/"),
        ),
    )
    .expect("sync path should reuse the standard package rerun hint");
    expect_stderr_not_contains(
        "project-check-sync-source-failure-context",
        "package-aware ql check sync with dependency source failures",
        &normalized_stderr,
        &format!(
            "hint: repair `{}` or rerun `ql project emit-interface {}` directly",
            dep_root
                .join("qlang.toml")
                .display()
                .to_string()
                .replace('\\', "/"),
            dep_root
                .join("qlang.toml")
                .display()
                .to_string()
                .replace('\\', "/")
        ),
    )
    .expect("sync path should not print the old duplicate direct-rerun hint");
    let package_note = format!(
        "note: failing package manifest: {}",
        dep_root
            .join("qlang.toml")
            .display()
            .to_string()
            .replace('\\', "/")
    );
    let owner_note = format!(
        "note: while syncing referenced package `../dep` from `{}`",
        app_manifest.display().to_string().replace('\\', "/")
    );
    let rerun_hint = format!(
        "hint: rerun `ql project emit-interface {}` after fixing the package sources",
        dep_root
            .join("qlang.toml")
            .display()
            .to_string()
            .replace('\\', "/"),
    );
    let package_note_index = normalized_stderr
        .find(&package_note)
        .expect("sync source failure should include the failing package manifest note");
    let owner_note_index = normalized_stderr
        .find(&owner_note)
        .expect("sync source failure should include the owner reference note");
    let rerun_hint_index = normalized_stderr
        .find(&rerun_hint)
        .expect("sync source failure should include the rerun hint");
    assert!(
        package_note_index < owner_note_index && owner_note_index < rerun_hint_index,
        "expected sync source failure context before rerun hint, got:\n{stderr}"
    );
    expect_stderr_contains(
        "project-check-sync-source-failure-context",
        "package-aware ql check sync with dependency source failures",
        &stderr,
        "`ql check --sync-interfaces` found 1 failing referenced package(s)",
    )
    .expect("sync path should still summarize the failing referenced package");
}

#[test]
fn check_package_dir_sync_interfaces_points_dependency_output_failures_at_interface_target() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-sync-output-path-failure");
    let dep_root = temp.path().join("workspace").join("dep");
    let app_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(dep_root.join("src"))
        .expect("create dependency source directory for sync output-path failure test");
    std::fs::create_dir_all(app_root.join("src"))
        .expect("create app source directory for sync output-path failure test");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "workspace/dep/src/lib.ql",
        r#"
package demo.dep

pub fn exported() -> Int {
    return 1
}
"#,
    );
    let interface_path = dep_root.join("dep.qi");
    std::fs::create_dir_all(&interface_path)
        .expect("create blocking interface directory for dependency sync test");
    let app_manifest = temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
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

    let dep_manifest = dep_root.join("qlang.toml");
    let dep_manifest_display = dep_manifest.to_string_lossy().replace('\\', "/");
    let interface_display = interface_path.to_string_lossy().replace('\\', "/");
    let app_manifest_display = app_manifest.to_string_lossy().replace('\\', "/");

    let mut command = ql_command(&workspace_root);
    command.args(["check", "--sync-interfaces"]).arg(&app_root);
    let output = run_command_capture(
        &mut command,
        "`ql check --sync-interfaces` dependency blocked output path",
    );
    let (_stdout, stderr) = expect_exit_code(
        "project-check-sync-output-path-failure",
        "package-aware ql check sync with dependency blocked output path",
        &output,
        1,
    )
    .expect("dependency output-path failures should fail package-aware sync");
    expect_stderr_contains(
        "project-check-sync-output-path-failure",
        "package-aware ql check sync with dependency blocked output path",
        &stderr,
        "failed to write interface",
    )
    .expect("sync path should surface the dependency write failure");
    let normalized_stderr = stderr.replace('\\', "/");
    let package_note = format!("note: failing package manifest: {dep_manifest_display}");
    let output_note = format!("note: failing interface output path: {interface_display}");
    let owner_note =
        format!("note: while syncing referenced package `../dep` from `{app_manifest_display}`");
    let rerun_hint = format!(
        "hint: rerun `ql project emit-interface {}` after fixing the interface output path",
        dep_manifest_display
    );
    let old_hint = format!(
        "hint: rerun `ql project emit-interface {}` after fixing the package interface error",
        dep_manifest_display
    );
    expect_stderr_contains(
        "project-check-sync-output-path-failure",
        "package-aware ql check sync with dependency blocked output path",
        &normalized_stderr,
        &package_note,
    )
    .expect("sync path should still point to the dependency manifest");
    expect_stderr_contains(
        "project-check-sync-output-path-failure",
        "package-aware ql check sync with dependency blocked output path",
        &normalized_stderr,
        &output_note,
    )
    .expect("sync path should point to the blocked dependency interface path");
    expect_stderr_contains(
        "project-check-sync-output-path-failure",
        "package-aware ql check sync with dependency blocked output path",
        &normalized_stderr,
        &owner_note,
    )
    .expect("sync path should still point back to the owner reference");
    expect_stderr_contains(
        "project-check-sync-output-path-failure",
        "package-aware ql check sync with dependency blocked output path",
        &normalized_stderr,
        &rerun_hint,
    )
    .expect("sync path should suggest fixing the dependency output path");
    expect_stderr_not_contains(
        "project-check-sync-output-path-failure",
        "package-aware ql check sync with dependency blocked output path",
        &normalized_stderr,
        &old_hint,
    )
    .expect("sync path should not reuse the source-failure rerun hint for output-path failures");
    let package_note_index = normalized_stderr
        .find(&package_note)
        .expect("sync output-path failure should include the dependency manifest note");
    let output_note_index = normalized_stderr
        .find(&output_note)
        .expect("sync output-path failure should include the blocked interface path note");
    let owner_note_index = normalized_stderr
        .find(&owner_note)
        .expect("sync output-path failure should include the owner reference note");
    let rerun_hint_index = normalized_stderr
        .find(&rerun_hint)
        .expect("sync output-path failure should include the rerun hint");
    assert!(
        package_note_index < output_note_index
            && output_note_index < owner_note_index
            && owner_note_index < rerun_hint_index,
        "expected sync output-path failure context before rerun hint, got:\n{stderr}"
    );
    expect_stderr_contains(
        "project-check-sync-output-path-failure",
        "package-aware ql check sync with dependency blocked output path",
        &stderr,
        "`ql check --sync-interfaces` found 1 failing referenced package(s)",
    )
    .expect("sync path should still summarize the failing referenced package");
    assert!(
        interface_path.is_dir(),
        "sync output-path failure test should preserve `{}` as a directory",
        interface_path.display()
    );
}

#[test]
fn check_package_dir_sync_interfaces_points_dependency_missing_source_root_at_source_root() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-sync-missing-dependency-source-root");
    let dep_root = temp.path().join("workspace").join("dep");
    let app_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(&dep_root)
        .expect("create dependency directory for sync missing source root test");
    std::fs::create_dir_all(app_root.join("src"))
        .expect("create app source directory for sync missing source root test");

    let dep_manifest = temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    let app_manifest = temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
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

    let dep_manifest_display = dep_manifest.to_string_lossy().replace('\\', "/");
    let dep_source_root_display = dep_root.join("src").to_string_lossy().replace('\\', "/");
    let app_manifest_display = app_manifest.to_string_lossy().replace('\\', "/");

    let mut command = ql_command(&workspace_root);
    command.args(["check", "--sync-interfaces"]).arg(&app_root);
    let output = run_command_capture(
        &mut command,
        "`ql check --sync-interfaces` dependency missing source root",
    );
    let (_stdout, stderr) = expect_exit_code(
        "project-check-sync-missing-dependency-source-root",
        "package-aware ql check sync with dependency missing source root",
        &output,
        1,
    )
    .expect("dependency missing source root should fail package-aware sync");
    let normalized_stderr = stderr.replace('\\', "/");
    let error_line = format!(
        "error: `ql check --sync-interfaces` package source directory `{dep_source_root_display}` does not exist"
    );
    let package_note = format!("note: failing package manifest: {dep_manifest_display}");
    let source_root_note = format!("note: failing package source root: {dep_source_root_display}");
    let owner_note =
        format!("note: while syncing referenced package `../dep` from `{app_manifest_display}`");
    let rerun_hint = format!(
        "hint: rerun `ql project emit-interface {dep_manifest_display}` after fixing the package source root"
    );
    let old_hint = format!(
        "hint: rerun `ql project emit-interface {dep_manifest_display}` after fixing the package interface error"
    );
    expect_stderr_contains(
        "project-check-sync-missing-dependency-source-root",
        "package-aware ql check sync with dependency missing source root",
        &normalized_stderr,
        &error_line,
    )
    .expect("sync path should preserve the command label for dependency source-root failures");
    expect_stderr_contains(
        "project-check-sync-missing-dependency-source-root",
        "package-aware ql check sync with dependency missing source root",
        &normalized_stderr,
        &package_note,
    )
    .expect("sync path should point to the dependency manifest");
    expect_stderr_contains(
        "project-check-sync-missing-dependency-source-root",
        "package-aware ql check sync with dependency missing source root",
        &normalized_stderr,
        &source_root_note,
    )
    .expect("sync path should point to the dependency source root");
    expect_stderr_contains(
        "project-check-sync-missing-dependency-source-root",
        "package-aware ql check sync with dependency missing source root",
        &normalized_stderr,
        &owner_note,
    )
    .expect("sync path should still point back to the owner reference");
    expect_stderr_contains(
        "project-check-sync-missing-dependency-source-root",
        "package-aware ql check sync with dependency missing source root",
        &normalized_stderr,
        &rerun_hint,
    )
    .expect("sync path should suggest fixing the dependency source root");
    expect_stderr_not_contains(
        "project-check-sync-missing-dependency-source-root",
        "package-aware ql check sync with dependency missing source root",
        &normalized_stderr,
        &old_hint,
    )
    .expect("sync path should not fall back to the generic dependency interface hint");
    let package_note_index = normalized_stderr
        .find(&package_note)
        .expect("sync missing source root should include the dependency manifest note");
    let source_root_note_index = normalized_stderr
        .find(&source_root_note)
        .expect("sync missing source root should include the dependency source root note");
    let owner_note_index = normalized_stderr
        .find(&owner_note)
        .expect("sync missing source root should include the owner reference note");
    let rerun_hint_index = normalized_stderr
        .find(&rerun_hint)
        .expect("sync missing source root should include the rerun hint");
    assert!(
        package_note_index < source_root_note_index
            && source_root_note_index < owner_note_index
            && owner_note_index < rerun_hint_index,
        "expected sync missing source-root context before rerun hint, got:\n{stderr}"
    );
    expect_stderr_contains(
        "project-check-sync-missing-dependency-source-root",
        "package-aware ql check sync with dependency missing source root",
        &stderr,
        "`ql check --sync-interfaces` found 1 failing referenced package(s)",
    )
    .expect("sync path should still summarize the failing referenced package");
}

#[test]
fn check_package_dir_reports_stale_dependency_interface() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-stale-interface");
    let dep_root = temp.path().join("workspace").join("dep");
    let app_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(dep_root.join("src")).expect("create dependency source directory");
    std::fs::create_dir_all(app_root.join("src")).expect("create app source directory");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "workspace/dep/src/lib.ql",
        r#"
package demo.dep

pub fn exported() -> Int {
    return 7
}
"#,
    );
    temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub fn exported() -> Int
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
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

    std::thread::sleep(std::time::Duration::from_millis(1200));
    temp.write(
        "workspace/dep/src/lib.ql",
        r#"
package demo.dep

pub fn exported() -> Int {
    return 9
}
"#,
    );

    let mut command = ql_command(&workspace_root);
    command.args(["check"]).arg(&app_root);
    let output = run_command_capture(&mut command, "`ql check` stale dependency interface");
    let (_stdout, stderr) = expect_exit_code(
        "project-check-stale-interface",
        "package-aware ql check with stale dependency interface",
        &output,
        1,
    )
    .expect("stale dependency interface should fail package-aware ql check");
    expect_stderr_contains(
        "project-check-stale-interface",
        "package-aware ql check with stale dependency interface",
        &stderr,
        "referenced package `dep` has stale interface artifact",
    )
    .expect("stale dependency interface should surface a clear error");
    expect_stderr_contains(
        "project-check-stale-interface",
        "package-aware ql check with stale dependency interface",
        &stderr,
        "reason: source newer than artifact:",
    )
    .expect("stale dependency interface should report why the artifact is stale");
    expect_stderr_contains(
        "project-check-stale-interface",
        "package-aware ql check with stale dependency interface",
        &stderr,
        "--sync-interfaces",
    )
    .expect("stale dependency interface diagnostic should suggest sync");
    let normalized_stderr = stderr.replace('\\', "/");
    let error_line = format!(
        "error: `ql check` referenced package `dep` has stale interface artifact `{}`",
        dep_root
            .join("dep.qi")
            .display()
            .to_string()
            .replace('\\', "/")
    );
    let old_error_line = format!(
        "error: referenced package `dep` has stale interface artifact `{}`",
        dep_root
            .join("dep.qi")
            .display()
            .to_string()
            .replace('\\', "/")
    );
    let reason_line = format!(
        "reason: source newer than artifact: {}",
        dep_root
            .join("src")
            .join("lib.ql")
            .display()
            .to_string()
            .replace('\\', "/")
    );
    let failing_manifest_note = format!(
        "note: failing referenced package manifest: {}",
        dep_root
            .join("qlang.toml")
            .display()
            .to_string()
            .replace('\\', "/")
    );
    let owner_note = format!(
        "note: while checking referenced package `../dep` from `{}`",
        app_root
            .join("qlang.toml")
            .display()
            .to_string()
            .replace('\\', "/")
    );
    let rerun_hint = format!(
        "hint: rerun `ql check --sync-interfaces {}` or regenerate `dep` with `ql project emit-interface {}`",
        app_root
            .join("qlang.toml")
            .display()
            .to_string()
            .replace('\\', "/"),
        dep_root
            .join("qlang.toml")
            .display()
            .to_string()
            .replace('\\', "/")
    );
    let error_index = normalized_stderr
        .find(&error_line)
        .expect("stale dependency interface should report the error line");
    expect_stderr_not_contains(
        "project-check-stale-interface",
        "package-aware ql check with stale dependency interface",
        &normalized_stderr,
        &old_error_line,
    )
    .expect("stale dependency interface should not fall back to the unlabeled artifact error");
    let reason_index = normalized_stderr
        .find(&reason_line)
        .expect("stale dependency interface should report the stale reason");
    let failing_manifest_index = normalized_stderr
        .find(&failing_manifest_note)
        .expect("stale dependency interface should point to the referenced manifest");
    let owner_note_index = normalized_stderr
        .find(&owner_note)
        .expect("stale dependency interface should point back to the owner manifest");
    let rerun_hint_index = normalized_stderr
        .find(&rerun_hint)
        .expect("stale dependency interface should include the repair hint");
    assert!(
        error_index < reason_index
            && reason_index < failing_manifest_index
            && failing_manifest_index < owner_note_index
            && owner_note_index < rerun_hint_index,
        "expected stale dependency interface diagnostic order error -> reason -> manifests -> hint, got:\n{stderr}"
    );
}

#[test]
fn check_package_dir_syncs_missing_dependency_interfaces() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-sync-interfaces");
    let dep_root = temp.path().join("workspace").join("dep");
    let app_root = temp.path().join("workspace").join("app");
    let source_path = app_root.join("src").join("lib.ql");
    let interface_path = dep_root.join("dep.qi");
    std::fs::create_dir_all(dep_root.join("src")).expect("create dependency source directory");
    std::fs::create_dir_all(app_root.join("src")).expect("create app source directory");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "workspace/dep/src/lib.ql",
        r#"
package demo.dep

pub fn exported() -> Int {
    return 7
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
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
    let output = run_command_capture(&mut command, "`ql check --sync-interfaces` package dir");
    let (stdout, stderr) = expect_success(
        "project-check-sync-interfaces",
        "package-aware ql check with synced dependency interfaces",
        &output,
    )
    .expect("syncing missing dependency interfaces should let package-aware ql check succeed");
    expect_stdout_contains_all(
        "project-check-sync-interfaces",
        &stdout,
        &[
            "wrote interface: ",
            "dep.qi",
            &format!("ok: {}", source_path.display()),
            "loaded interface: ",
        ],
    )
    .expect("syncing missing dependency interfaces should report emitted and loaded interfaces");
    assert!(
        interface_path.is_file(),
        "expected synced dependency interface at `{}`",
        interface_path.display()
    );
    assert!(
        stderr.trim().is_empty(),
        "expected package-aware ql check stderr to stay empty, got:\n{stderr}"
    );
}

#[test]
fn check_package_dir_syncs_stale_dependency_interfaces() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-sync-stale-interfaces");
    let dep_root = temp.path().join("workspace").join("dep");
    let app_root = temp.path().join("workspace").join("app");
    let source_path = app_root.join("src").join("lib.ql");
    let interface_path = dep_root.join("dep.qi");
    std::fs::create_dir_all(dep_root.join("src")).expect("create dependency source directory");
    std::fs::create_dir_all(app_root.join("src")).expect("create app source directory");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "workspace/dep/src/lib.ql",
        r#"
package demo.dep

pub fn exported() -> Int {
    return 7
}
"#,
    );
    temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub fn exported() -> Int
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
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

    std::thread::sleep(std::time::Duration::from_millis(1200));
    temp.write(
        "workspace/dep/src/lib.ql",
        r#"
package demo.dep

pub fn exported() -> Int {
    return 9
}
"#,
    );

    let mut command = ql_command(&workspace_root);
    command.args(["check", "--sync-interfaces"]).arg(&app_root);
    let output = run_command_capture(
        &mut command,
        "`ql check --sync-interfaces` stale package dir",
    );
    let (stdout, stderr) = expect_success(
        "project-check-sync-stale-interfaces",
        "package-aware ql check with synced stale dependency interfaces",
        &output,
    )
    .expect("syncing stale dependency interfaces should let package-aware ql check succeed");
    expect_stdout_contains_all(
        "project-check-sync-stale-interfaces",
        &stdout,
        &[
            "wrote interface: ",
            "dep.qi",
            &format!("ok: {}", source_path.display()),
            "loaded interface: ",
        ],
    )
    .expect("syncing stale dependency interfaces should report emitted and loaded interfaces");
    assert!(
        interface_path.is_file(),
        "expected synced dependency interface at `{}`",
        interface_path.display()
    );
    assert!(
        stderr.trim().is_empty(),
        "expected package-aware ql check stderr to stay empty, got:\n{stderr}"
    );
}

#[test]
fn check_source_file_syncs_missing_dependency_interfaces() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-sync-interfaces-source-file");
    let dep_root = temp.path().join("workspace").join("dep");
    let app_root = temp.path().join("workspace").join("app");
    let source_path = app_root.join("src").join("lib.ql");
    let interface_path = dep_root.join("dep.qi");
    std::fs::create_dir_all(dep_root.join("src")).expect("create dependency source directory");
    std::fs::create_dir_all(app_root.join("src")).expect("create app source directory");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "workspace/dep/src/lib.ql",
        r#"
package demo.dep

pub fn exported() -> Int {
    return 7
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
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
    command
        .args(["check", "--sync-interfaces"])
        .arg(&source_path);
    let output = run_command_capture(&mut command, "`ql check --sync-interfaces` source file");
    let (stdout, stderr) = expect_success(
        "project-check-sync-interfaces-source-file",
        "package-aware ql check with synced dependency interfaces from source file",
        &output,
    )
    .expect("syncing interfaces from a source file path should let package-aware ql check succeed");
    expect_stdout_contains_all(
        "project-check-sync-interfaces-source-file",
        &stdout,
        &[
            "wrote interface: ",
            "dep.qi",
            &format!("ok: {}", source_path.display()),
            "loaded interface: ",
        ],
    )
    .expect("source-file sync path should report emitted and loaded interfaces");
    assert!(
        interface_path.is_file(),
        "expected synced dependency interface at `{}`",
        interface_path.display()
    );
    assert!(
        stderr.trim().is_empty(),
        "expected package-aware ql check stderr to stay empty, got:\n{stderr}"
    );
}

#[test]
fn check_workspace_root_runs_member_packages() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-workspace-root");
    let dep_root = temp.path().join("workspace").join("dep");
    let app_root = temp.path().join("workspace").join("packages").join("app");
    let tool_root = temp.path().join("workspace").join("packages").join("tool");
    let app_source = app_root.join("src").join("lib.ql");
    let tool_source = tool_root.join("src").join("lib.ql");
    let workspace_manifest = temp.path().join("workspace");
    std::fs::create_dir_all(dep_root.join("src")).expect("create dependency source directory");
    std::fs::create_dir_all(app_root.join("src")).expect("create app source directory");
    std::fs::create_dir_all(tool_root.join("src")).expect("create tool source directory");

    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/tool"]
"#,
    );
    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub fn exported() -> Int
"#,
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../../dep"]
"#,
    );
    temp.write(
        "workspace/packages/app/src/lib.ql",
        r#"
package demo.app

pub fn main() -> Int {
    return 1
}
"#,
    );
    temp.write(
        "workspace/packages/tool/qlang.toml",
        r#"
[package]
name = "tool"
"#,
    );
    temp.write(
        "workspace/packages/tool/src/lib.ql",
        r#"
package demo.tool

pub fn main() -> Int {
    return 2
}
"#,
    );

    let mut command = ql_command(&workspace_root);
    command.args(["check"]).arg(&workspace_manifest);
    let output = run_command_capture(&mut command, "`ql check` workspace root");
    let (stdout, stderr) = expect_success(
        "project-check-workspace-root",
        "workspace-root ql check",
        &output,
    )
    .expect("workspace-root ql check should succeed");
    let normalized_stdout = stdout.replace('\\', "/");
    expect_stdout_contains_all(
        "project-check-workspace-root",
        &normalized_stdout,
        &[
            &format!(
                "ok: {}",
                app_source.display().to_string().replace('\\', "/")
            ),
            &format!(
                "ok: {}",
                tool_source.display().to_string().replace('\\', "/")
            ),
            "loaded interface: ",
            "dep.qi",
        ],
    )
    .expect("workspace-root ql check should report member sources and dependency interfaces");
    assert!(
        stderr.trim().is_empty(),
        "expected workspace-root ql check stderr to stay empty, got:\n{stderr}"
    );
}

#[test]
fn check_workspace_member_source_file_uses_enclosing_workspace() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-workspace-member-source");
    let dep_root = temp.path().join("workspace").join("dep");
    let app_root = temp.path().join("workspace").join("packages").join("app");
    let tool_root = temp.path().join("workspace").join("packages").join("tool");
    let app_source = app_root.join("src").join("lib.ql");
    let tool_source = tool_root.join("src").join("lib.ql");
    std::fs::create_dir_all(dep_root.join("src")).expect("create dependency source directory");
    std::fs::create_dir_all(app_root.join("src")).expect("create app source directory");
    std::fs::create_dir_all(tool_root.join("src")).expect("create tool source directory");

    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/tool"]
"#,
    );
    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub fn exported() -> Int
"#,
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../../dep"]
"#,
    );
    temp.write(
        "workspace/packages/app/src/lib.ql",
        r#"
package demo.app

pub fn main() -> Int {
    return 1
}
"#,
    );
    temp.write(
        "workspace/packages/tool/qlang.toml",
        r#"
[package]
name = "tool"
"#,
    );
    temp.write(
        "workspace/packages/tool/src/lib.ql",
        r#"
package demo.tool

pub fn main() -> Int {
    return 2
}
"#,
    );

    let mut command = ql_command(&workspace_root);
    command.args(["check"]).arg(&app_source);
    let output = run_command_capture(&mut command, "`ql check` workspace member source file");
    let (stdout, stderr) = expect_success(
        "project-check-workspace-member-source",
        "workspace member source ql check",
        &output,
    )
    .expect("workspace member source ql check should succeed");
    let normalized_stdout = stdout.replace('\\', "/");
    expect_stdout_contains_all(
        "project-check-workspace-member-source",
        &normalized_stdout,
        &[
            &format!("ok: {}", app_source.display().to_string().replace('\\', "/")),
            &format!("ok: {}", tool_source.display().to_string().replace('\\', "/")),
            "loaded interface: ",
            "dep.qi",
        ],
    )
    .expect(
        "workspace member source ql check should reuse the enclosing workspace instead of only the member package",
    );
    assert!(
        stderr.trim().is_empty(),
        "expected workspace member source ql check stderr to stay empty, got:\n{stderr}"
    );
}

#[test]
fn check_workspace_root_syncs_dependency_interfaces_once() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-workspace-sync");
    let dep_root = temp.path().join("workspace").join("dep");
    let app_root = temp.path().join("workspace").join("packages").join("app");
    let tool_root = temp.path().join("workspace").join("packages").join("tool");
    let app_source = app_root.join("src").join("lib.ql");
    let tool_source = tool_root.join("src").join("lib.ql");
    let interface_path = dep_root.join("dep.qi");
    let workspace_manifest = temp.path().join("workspace");
    std::fs::create_dir_all(dep_root.join("src")).expect("create dependency source directory");
    std::fs::create_dir_all(app_root.join("src")).expect("create app source directory");
    std::fs::create_dir_all(tool_root.join("src")).expect("create tool source directory");

    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/tool"]
"#,
    );
    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "workspace/dep/src/lib.ql",
        r#"
package demo.dep

pub fn exported() -> Int {
    return 7
}
"#,
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../../dep"]
"#,
    );
    temp.write(
        "workspace/packages/app/src/lib.ql",
        r#"
package demo.app

pub fn main() -> Int {
    return 1
}
"#,
    );
    temp.write(
        "workspace/packages/tool/qlang.toml",
        r#"
[package]
name = "tool"

[references]
packages = ["../../dep"]
"#,
    );
    temp.write(
        "workspace/packages/tool/src/lib.ql",
        r#"
package demo.tool

pub fn main() -> Int {
    return 2
}
"#,
    );

    let mut command = ql_command(&workspace_root);
    command
        .args(["check", "--sync-interfaces"])
        .arg(&workspace_manifest);
    let output = run_command_capture(&mut command, "`ql check --sync-interfaces` workspace root");
    let (stdout, stderr) = expect_success(
        "project-check-workspace-sync",
        "workspace-root ql check with synced dependency interfaces",
        &output,
    )
    .expect("workspace-root ql check with synced dependency interfaces should succeed");
    let normalized_stdout = stdout.replace('\\', "/");
    expect_stdout_contains_all(
        "project-check-workspace-sync",
        &normalized_stdout,
        &[
            "wrote interface: ",
            "dep.qi",
            &format!(
                "ok: {}",
                app_source.display().to_string().replace('\\', "/")
            ),
            &format!(
                "ok: {}",
                tool_source.display().to_string().replace('\\', "/")
            ),
            "loaded interface: ",
        ],
    )
    .expect("workspace-root sync path should report emitted and loaded interfaces");
    assert_eq!(
        normalized_stdout.matches("wrote interface: ").count(),
        1,
        "expected workspace-root sync path to emit one dependency interface, got:\n{stdout}"
    );
    assert!(
        interface_path.is_file(),
        "expected synced dependency interface at `{}`",
        interface_path.display()
    );
    assert!(
        stderr.trim().is_empty(),
        "expected workspace-root ql check stderr to stay empty, got:\n{stderr}"
    );
}

#[test]
fn check_workspace_root_dedupes_single_failing_member_summary() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-workspace-single-failure");
    let app_root = temp.path().join("workspace").join("packages").join("app");
    let broken_root = temp
        .path()
        .join("workspace")
        .join("packages")
        .join("broken");
    let app_source = app_root.join("src").join("lib.ql");
    let workspace_manifest = temp.path().join("workspace");
    std::fs::create_dir_all(app_root.join("src")).expect("create app source directory");
    std::fs::create_dir_all(&broken_root).expect("create broken member directory");

    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/broken"]
"#,
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace/packages/app/src/lib.ql",
        r#"
package demo.app

pub fn main() -> Int {
    return 1
}
"#,
    );
    temp.write(
        "workspace/packages/broken/qlang.toml",
        r#"
[package
name = "broken"
"#,
    );

    let mut command = ql_command(&workspace_root);
    command.args(["check"]).arg(&workspace_manifest);
    let output = run_command_capture(
        &mut command,
        "`ql check` workspace root with single failing member",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-check-workspace-single-failure",
        "workspace-root ql check with single failing member",
        &output,
        1,
    )
    .expect("workspace-root ql check with a single failing member should fail");
    let normalized_stdout = stdout.replace('\\', "/");
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stdout_contains_all(
        "project-check-workspace-single-failure",
        &normalized_stdout,
        &[&format!(
            "ok: {}",
            app_source.display().to_string().replace('\\', "/")
        )],
    )
    .expect("workspace-root ql check should still report healthy members before failing");
    expect_stderr_contains(
        "project-check-workspace-single-failure",
        "workspace-root ql check with single failing member",
        &normalized_stderr,
        &format!(
            "error: `ql check` invalid manifest `{}`",
            broken_root
                .join("qlang.toml")
                .display()
                .to_string()
                .replace('\\', "/")
        ),
    )
    .expect(
        "workspace-root ql check should preserve the command label for broken member manifests",
    );
    expect_stderr_not_contains(
        "project-check-workspace-single-failure",
        "workspace-root ql check with single failing member",
        &normalized_stderr,
        &format!(
            "error: invalid manifest `{}`",
            broken_root
                .join("qlang.toml")
                .display()
                .to_string()
                .replace('\\', "/")
        ),
    )
    .expect("workspace-root ql check should not fall back to the generic broken member error line");
    expect_stderr_contains(
        "project-check-workspace-single-failure",
        "workspace-root ql check with single failing member",
        &normalized_stderr,
        &format!(
            "note: failing package manifest: {}",
            broken_root
                .join("qlang.toml")
                .display()
                .to_string()
                .replace('\\', "/")
        ),
    )
    .expect("workspace-root ql check should point the broken member at the package manifest");
    expect_stderr_contains(
        "project-check-workspace-single-failure",
        "workspace-root ql check with single failing member",
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
    .expect("workspace-root ql check should point the broken member locally");
    let rerun_hint = format!(
        "hint: rerun `ql check {}` after fixing the package manifest",
        broken_root
            .join("qlang.toml")
            .display()
            .to_string()
            .replace('\\', "/")
    );
    expect_stderr_contains(
        "project-check-workspace-single-failure",
        "workspace-root ql check with single failing member",
        &normalized_stderr,
        &rerun_hint,
    )
    .expect(
        "workspace-root ql check should suggest rerunning the broken member after fixing the package manifest",
    );
    expect_stderr_contains(
        "project-check-workspace-single-failure",
        "workspace-root ql check with single failing member",
        &stderr,
        "`ql check` found 1 failing member(s)",
    )
    .expect("workspace-root ql check should summarize the single failing member");
    expect_stderr_not_contains(
        "project-check-workspace-single-failure",
        "workspace-root ql check with single failing member",
        &normalized_stderr,
        "note: first failing member manifest:",
    )
    .expect("single failing workspace members should not repeat the manifest in the final summary");
}

#[test]
fn check_workspace_root_sync_preserves_broken_member_manifest_label() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-workspace-sync-single-failure");
    let app_root = temp.path().join("workspace").join("packages").join("app");
    let broken_root = temp
        .path()
        .join("workspace")
        .join("packages")
        .join("broken");
    let app_source = app_root.join("src").join("lib.ql");
    let workspace_manifest = temp.path().join("workspace");
    std::fs::create_dir_all(app_root.join("src")).expect("create app source directory");
    std::fs::create_dir_all(&broken_root).expect("create broken member directory");

    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/broken"]
"#,
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace/packages/app/src/lib.ql",
        r#"
package demo.app

pub fn main() -> Int {
    return 1
}
"#,
    );
    temp.write(
        "workspace/packages/broken/qlang.toml",
        r#"
[package
name = "broken"
"#,
    );

    let mut command = ql_command(&workspace_root);
    command
        .args(["check", "--sync-interfaces"])
        .arg(&workspace_manifest);
    let output = run_command_capture(
        &mut command,
        "`ql check --sync-interfaces` workspace root with single failing member",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-check-workspace-sync-single-failure",
        "workspace-root ql check sync with single failing member",
        &output,
        1,
    )
    .expect("workspace-root ql check sync with a single failing member should fail");
    let normalized_stdout = stdout.replace('\\', "/");
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stdout_contains_all(
        "project-check-workspace-sync-single-failure",
        &normalized_stdout,
        &[&format!(
            "ok: {}",
            app_source.display().to_string().replace('\\', "/")
        )],
    )
    .expect("workspace-root ql check sync should still report healthy members before failing");
    let broken_manifest = broken_root
        .join("qlang.toml")
        .display()
        .to_string()
        .replace('\\', "/");
    let error_line =
        format!("error: `ql check --sync-interfaces` invalid manifest `{broken_manifest}`");
    let old_error_line = format!("error: invalid manifest `{broken_manifest}`");
    let package_note = format!("note: failing package manifest: {broken_manifest}");
    let member_note = format!("note: failing workspace member manifest: {broken_manifest}");
    let rerun_hint = format!(
        "hint: rerun `ql check --sync-interfaces {broken_manifest}` after fixing the package manifest"
    );
    expect_stderr_contains(
        "project-check-workspace-sync-single-failure",
        "workspace-root ql check sync with single failing member",
        &normalized_stderr,
        &error_line,
    )
    .expect("workspace-root ql check sync should preserve the command label for broken member manifests");
    expect_stderr_not_contains(
        "project-check-workspace-sync-single-failure",
        "workspace-root ql check sync with single failing member",
        &normalized_stderr,
        &old_error_line,
    )
    .expect(
        "workspace-root ql check sync should not fall back to the generic broken member error line",
    );
    expect_stderr_contains(
        "project-check-workspace-sync-single-failure",
        "workspace-root ql check sync with single failing member",
        &normalized_stderr,
        &package_note,
    )
    .expect("workspace-root ql check sync should point the broken member at the package manifest");
    expect_stderr_contains(
        "project-check-workspace-sync-single-failure",
        "workspace-root ql check sync with single failing member",
        &normalized_stderr,
        &member_note,
    )
    .expect("workspace-root ql check sync should point the broken member locally");
    expect_stderr_contains(
        "project-check-workspace-sync-single-failure",
        "workspace-root ql check sync with single failing member",
        &normalized_stderr,
        &rerun_hint,
    )
    .expect("workspace-root ql check sync should suggest rerunning the broken member after fixing the package manifest");
    expect_stderr_contains(
        "project-check-workspace-sync-single-failure",
        "workspace-root ql check sync with single failing member",
        &stderr,
        "`ql check --sync-interfaces` found 1 failing member(s)",
    )
    .expect("workspace-root ql check sync should summarize the single failing member");
    expect_stderr_not_contains(
        "project-check-workspace-sync-single-failure",
        "workspace-root ql check sync with single failing member",
        &normalized_stderr,
        "note: first failing member manifest:",
    )
    .expect(
        "single failing sync workspace members should not repeat the manifest in the final summary",
    );
}

#[test]
fn check_workspace_root_preserves_non_package_member_rerun_hint() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-workspace-non-package-member");
    let app_root = temp.path().join("workspace").join("packages").join("app");
    let broken_root = temp
        .path()
        .join("workspace")
        .join("packages")
        .join("broken");
    let tool_root = temp.path().join("workspace").join("packages").join("tool");
    let app_source = app_root.join("src").join("lib.ql");
    let tool_source = tool_root.join("src").join("lib.ql");
    let workspace_manifest = temp.path().join("workspace");
    std::fs::create_dir_all(app_root.join("src")).expect("create app source directory");
    std::fs::create_dir_all(&broken_root).expect("create broken member directory");
    std::fs::create_dir_all(tool_root.join("src")).expect("create tool source directory");

    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/broken", "packages/tool"]
"#,
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace/packages/app/src/lib.ql",
        r#"
package demo.app

pub fn main() -> Int {
    return 1
}
"#,
    );
    temp.write(
        "workspace/packages/broken/qlang.toml",
        r#"
[workspace]
members = []
"#,
    );
    temp.write(
        "workspace/packages/tool/qlang.toml",
        r#"
[package]
name = "tool"
"#,
    );
    temp.write(
        "workspace/packages/tool/src/lib.ql",
        r#"
package demo.tool

pub fn main() -> Int {
    return 2
}
"#,
    );

    let mut command = ql_command(&workspace_root);
    command.args(["check"]).arg(&workspace_manifest);
    let output = run_command_capture(
        &mut command,
        "`ql check` workspace root with non-package member",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-check-workspace-non-package-member",
        "workspace-root ql check with non-package member",
        &output,
        1,
    )
    .expect("workspace-root ql check with non-package member should fail");
    let normalized_stdout = stdout.replace('\\', "/");
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stdout_contains_all(
        "project-check-workspace-non-package-member",
        &normalized_stdout,
        &[
            &format!(
                "ok: {}",
                app_source.display().to_string().replace('\\', "/")
            ),
            &format!(
                "ok: {}",
                tool_source.display().to_string().replace('\\', "/")
            ),
        ],
    )
    .expect("workspace-root ql check should continue checking later valid members");
    let broken_manifest = broken_root
        .join("qlang.toml")
        .display()
        .to_string()
        .replace('\\', "/");
    let error_line =
        format!("error: `ql check` manifest `{broken_manifest}` does not declare `[package].name`");
    let old_error_line =
        format!("error: manifest `{broken_manifest}` does not declare `[package].name`");
    let package_note = format!("note: failing package manifest: {broken_manifest}");
    let member_note = format!("note: failing workspace member manifest: {broken_manifest}");
    let rerun_hint =
        format!("hint: rerun `ql check {broken_manifest}` after fixing the package manifest");
    expect_stderr_contains(
        "project-check-workspace-non-package-member",
        "workspace-root ql check with non-package member",
        &normalized_stderr,
        &error_line,
    )
    .expect(
        "workspace-root ql check should preserve the direct command label for non-package members",
    );
    expect_stderr_not_contains(
        "project-check-workspace-non-package-member",
        "workspace-root ql check with non-package member",
        &normalized_stderr,
        &old_error_line,
    )
    .expect("workspace-root ql check should not fall back to the generic non-package error line");
    let error_line_index = normalized_stderr
        .find(&error_line)
        .expect("workspace-root ql check should include the non-package member error");
    let package_note_index = normalized_stderr
        .find(&package_note)
        .expect("workspace-root ql check should include the package manifest note");
    let member_note_index = normalized_stderr
        .find(&member_note)
        .expect("workspace-root ql check should include the local member note");
    let rerun_hint_index = normalized_stderr
        .find(&rerun_hint)
        .expect("workspace-root ql check should include the rerun hint");
    assert!(
        error_line_index < package_note_index
            && package_note_index < member_note_index
            && member_note_index < rerun_hint_index,
        "expected non-package member context before rerun hint, got:\n{stderr}"
    );
    expect_stderr_contains(
        "project-check-workspace-non-package-member",
        "workspace-root ql check with non-package member",
        &stderr,
        "`ql check` found 1 failing member(s)",
    )
    .expect("workspace-root ql check should summarize the single non-package member");
    expect_stderr_not_contains(
        "project-check-workspace-non-package-member",
        "workspace-root ql check with non-package member",
        &normalized_stderr,
        "note: first failing member manifest:",
    )
    .expect(
        "single non-package workspace members should not repeat the manifest in the final summary",
    );
}

#[test]
fn check_workspace_root_sync_preserves_non_package_member_rerun_hint() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-workspace-sync-non-package-member");
    let app_root = temp.path().join("workspace").join("packages").join("app");
    let broken_root = temp
        .path()
        .join("workspace")
        .join("packages")
        .join("broken");
    let tool_root = temp.path().join("workspace").join("packages").join("tool");
    let app_source = app_root.join("src").join("lib.ql");
    let tool_source = tool_root.join("src").join("lib.ql");
    let workspace_manifest = temp.path().join("workspace");
    std::fs::create_dir_all(app_root.join("src")).expect("create app source directory");
    std::fs::create_dir_all(&broken_root).expect("create broken member directory");
    std::fs::create_dir_all(tool_root.join("src")).expect("create tool source directory");

    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/broken", "packages/tool"]
"#,
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace/packages/app/src/lib.ql",
        r#"
package demo.app

pub fn main() -> Int {
    return 1
}
"#,
    );
    temp.write(
        "workspace/packages/broken/qlang.toml",
        r#"
[workspace]
members = []
"#,
    );
    temp.write(
        "workspace/packages/tool/qlang.toml",
        r#"
[package]
name = "tool"
"#,
    );
    temp.write(
        "workspace/packages/tool/src/lib.ql",
        r#"
package demo.tool

pub fn main() -> Int {
    return 2
}
"#,
    );

    let mut command = ql_command(&workspace_root);
    command
        .args(["check", "--sync-interfaces"])
        .arg(&workspace_manifest);
    let output = run_command_capture(
        &mut command,
        "`ql check --sync-interfaces` workspace root with non-package member",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-check-workspace-sync-non-package-member",
        "workspace-root ql check sync with non-package member",
        &output,
        1,
    )
    .expect("workspace-root ql check sync with non-package member should fail");
    let normalized_stdout = stdout.replace('\\', "/");
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stdout_contains_all(
        "project-check-workspace-sync-non-package-member",
        &normalized_stdout,
        &[
            &format!(
                "ok: {}",
                app_source.display().to_string().replace('\\', "/")
            ),
            &format!(
                "ok: {}",
                tool_source.display().to_string().replace('\\', "/")
            ),
        ],
    )
    .expect("workspace-root ql check sync should continue checking later valid members");
    let broken_manifest = broken_root
        .join("qlang.toml")
        .display()
        .to_string()
        .replace('\\', "/");
    let error_line = format!(
        "error: `ql check --sync-interfaces` manifest `{broken_manifest}` does not declare `[package].name`"
    );
    let old_error_line =
        format!("error: manifest `{broken_manifest}` does not declare `[package].name`");
    let package_note = format!("note: failing package manifest: {broken_manifest}");
    let member_note = format!("note: failing workspace member manifest: {broken_manifest}");
    let rerun_hint = format!(
        "hint: rerun `ql check --sync-interfaces {broken_manifest}` after fixing the package manifest"
    );
    expect_stderr_contains(
        "project-check-workspace-sync-non-package-member",
        "workspace-root ql check sync with non-package member",
        &normalized_stderr,
        &error_line,
    )
    .expect("workspace-root ql check sync should preserve the direct command label for non-package members");
    expect_stderr_not_contains(
        "project-check-workspace-sync-non-package-member",
        "workspace-root ql check sync with non-package member",
        &normalized_stderr,
        &old_error_line,
    )
    .expect(
        "workspace-root ql check sync should not fall back to the generic non-package error line",
    );
    let error_line_index = normalized_stderr
        .find(&error_line)
        .expect("workspace-root ql check sync should include the non-package member error");
    let package_note_index = normalized_stderr
        .find(&package_note)
        .expect("workspace-root ql check sync should include the package manifest note");
    let member_note_index = normalized_stderr
        .find(&member_note)
        .expect("workspace-root ql check sync should include the local member note");
    let rerun_hint_index = normalized_stderr
        .find(&rerun_hint)
        .expect("workspace-root ql check sync should include the rerun hint");
    assert!(
        error_line_index < package_note_index
            && package_note_index < member_note_index
            && member_note_index < rerun_hint_index,
        "expected sync non-package member context before rerun hint, got:\n{stderr}"
    );
    expect_stderr_contains(
        "project-check-workspace-sync-non-package-member",
        "workspace-root ql check sync with non-package member",
        &stderr,
        "`ql check --sync-interfaces` found 1 failing member(s)",
    )
    .expect("workspace-root ql check sync should summarize the single non-package member");
    expect_stderr_not_contains(
        "project-check-workspace-sync-non-package-member",
        "workspace-root ql check sync with non-package member",
        &normalized_stderr,
        "note: first failing member manifest:",
    )
    .expect("single sync non-package workspace members should not repeat the manifest in the final summary");
}

#[test]
fn check_workspace_root_preserves_missing_member_package_name_rerun_hint() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-workspace-missing-member-package-name");
    let app_root = temp.path().join("workspace").join("packages").join("app");
    let broken_root = temp
        .path()
        .join("workspace")
        .join("packages")
        .join("broken");
    let tool_root = temp.path().join("workspace").join("packages").join("tool");
    let app_source = app_root.join("src").join("lib.ql");
    let tool_source = tool_root.join("src").join("lib.ql");
    let workspace_manifest = temp.path().join("workspace");
    std::fs::create_dir_all(app_root.join("src")).expect("create app source directory");
    std::fs::create_dir_all(&broken_root).expect("create broken member directory");
    std::fs::create_dir_all(tool_root.join("src")).expect("create tool source directory");

    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/broken", "packages/tool"]
"#,
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace/packages/app/src/lib.ql",
        r#"
package demo.app

pub fn main() -> Int {
    return 1
}
"#,
    );
    temp.write(
        "workspace/packages/broken/qlang.toml",
        r#"
[package]
version = "0.1.0"
"#,
    );
    temp.write(
        "workspace/packages/tool/qlang.toml",
        r#"
[package]
name = "tool"
"#,
    );
    temp.write(
        "workspace/packages/tool/src/lib.ql",
        r#"
package demo.tool

pub fn main() -> Int {
    return 2
}
"#,
    );

    let mut command = ql_command(&workspace_root);
    command.args(["check"]).arg(&workspace_manifest);
    let output = run_command_capture(
        &mut command,
        "`ql check` workspace root with missing member package name",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-check-workspace-missing-member-package-name",
        "workspace-root ql check with missing member package name",
        &output,
        1,
    )
    .expect("workspace-root ql check with missing member package name should fail");
    let normalized_stdout = stdout.replace('\\', "/");
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stdout_contains_all(
        "project-check-workspace-missing-member-package-name",
        &normalized_stdout,
        &[
            &format!(
                "ok: {}",
                app_source.display().to_string().replace('\\', "/")
            ),
            &format!(
                "ok: {}",
                tool_source.display().to_string().replace('\\', "/")
            ),
        ],
    )
    .expect("workspace-root ql check should continue checking later valid members");
    let broken_manifest = broken_root
        .join("qlang.toml")
        .display()
        .to_string()
        .replace('\\', "/");
    let error_line =
        format!("error: `ql check` manifest `{broken_manifest}` does not declare `[package].name`");
    let old_error_line =
        format!("error: manifest `{broken_manifest}` does not declare `[package].name`");
    let package_note = format!("note: failing package manifest: {broken_manifest}");
    let member_note = format!("note: failing workspace member manifest: {broken_manifest}");
    let rerun_hint =
        format!("hint: rerun `ql check {broken_manifest}` after fixing the package manifest");
    expect_stderr_contains(
        "project-check-workspace-missing-member-package-name",
        "workspace-root ql check with missing member package name",
        &normalized_stderr,
        &error_line,
    )
    .expect(
        "workspace-root ql check should preserve the direct command label for missing member package names",
    );
    expect_stderr_not_contains(
        "project-check-workspace-missing-member-package-name",
        "workspace-root ql check with missing member package name",
        &normalized_stderr,
        &old_error_line,
    )
    .expect(
        "workspace-root ql check should not fall back to the generic missing member package-name error line",
    );
    let error_line_index = normalized_stderr
        .find(&error_line)
        .expect("workspace-root ql check should include the missing member package-name error");
    let package_note_index = normalized_stderr
        .find(&package_note)
        .expect("workspace-root ql check should include the package manifest note");
    let member_note_index = normalized_stderr
        .find(&member_note)
        .expect("workspace-root ql check should include the local member note");
    let rerun_hint_index = normalized_stderr
        .find(&rerun_hint)
        .expect("workspace-root ql check should include the rerun hint");
    assert!(
        error_line_index < package_note_index
            && package_note_index < member_note_index
            && member_note_index < rerun_hint_index,
        "expected missing member package-name context before rerun hint, got:\n{stderr}"
    );
    expect_stderr_contains(
        "project-check-workspace-missing-member-package-name",
        "workspace-root ql check with missing member package name",
        &stderr,
        "`ql check` found 1 failing member(s)",
    )
    .expect("workspace-root ql check should summarize the single missing member package name");
    expect_stderr_not_contains(
        "project-check-workspace-missing-member-package-name",
        "workspace-root ql check with missing member package name",
        &normalized_stderr,
        "note: first failing member manifest:",
    )
    .expect(
        "single missing member package-name failures should not repeat the manifest in the final summary",
    );
}

#[test]
fn check_workspace_root_sync_preserves_missing_member_package_name_rerun_hint() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-workspace-sync-missing-member-package-name");
    let app_root = temp.path().join("workspace").join("packages").join("app");
    let broken_root = temp
        .path()
        .join("workspace")
        .join("packages")
        .join("broken");
    let tool_root = temp.path().join("workspace").join("packages").join("tool");
    let app_source = app_root.join("src").join("lib.ql");
    let tool_source = tool_root.join("src").join("lib.ql");
    let workspace_manifest = temp.path().join("workspace");
    std::fs::create_dir_all(app_root.join("src")).expect("create app source directory");
    std::fs::create_dir_all(&broken_root).expect("create broken member directory");
    std::fs::create_dir_all(tool_root.join("src")).expect("create tool source directory");

    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/broken", "packages/tool"]
"#,
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace/packages/app/src/lib.ql",
        r#"
package demo.app

pub fn main() -> Int {
    return 1
}
"#,
    );
    temp.write(
        "workspace/packages/broken/qlang.toml",
        r#"
[package]
version = "0.1.0"
"#,
    );
    temp.write(
        "workspace/packages/tool/qlang.toml",
        r#"
[package]
name = "tool"
"#,
    );
    temp.write(
        "workspace/packages/tool/src/lib.ql",
        r#"
package demo.tool

pub fn main() -> Int {
    return 2
}
"#,
    );

    let mut command = ql_command(&workspace_root);
    command
        .args(["check", "--sync-interfaces"])
        .arg(&workspace_manifest);
    let output = run_command_capture(
        &mut command,
        "`ql check --sync-interfaces` workspace root with missing member package name",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-check-workspace-sync-missing-member-package-name",
        "workspace-root ql check sync with missing member package name",
        &output,
        1,
    )
    .expect("workspace-root ql check sync with missing member package name should fail");
    let normalized_stdout = stdout.replace('\\', "/");
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stdout_contains_all(
        "project-check-workspace-sync-missing-member-package-name",
        &normalized_stdout,
        &[
            &format!(
                "ok: {}",
                app_source.display().to_string().replace('\\', "/")
            ),
            &format!(
                "ok: {}",
                tool_source.display().to_string().replace('\\', "/")
            ),
        ],
    )
    .expect("workspace-root ql check sync should continue checking later valid members");
    let broken_manifest = broken_root
        .join("qlang.toml")
        .display()
        .to_string()
        .replace('\\', "/");
    let error_line = format!(
        "error: `ql check --sync-interfaces` manifest `{broken_manifest}` does not declare `[package].name`"
    );
    let old_error_line =
        format!("error: manifest `{broken_manifest}` does not declare `[package].name`");
    let package_note = format!("note: failing package manifest: {broken_manifest}");
    let member_note = format!("note: failing workspace member manifest: {broken_manifest}");
    let rerun_hint = format!(
        "hint: rerun `ql check --sync-interfaces {broken_manifest}` after fixing the package manifest"
    );
    expect_stderr_contains(
        "project-check-workspace-sync-missing-member-package-name",
        "workspace-root ql check sync with missing member package name",
        &normalized_stderr,
        &error_line,
    )
    .expect(
        "workspace-root ql check sync should preserve the direct command label for missing member package names",
    );
    expect_stderr_not_contains(
        "project-check-workspace-sync-missing-member-package-name",
        "workspace-root ql check sync with missing member package name",
        &normalized_stderr,
        &old_error_line,
    )
    .expect(
        "workspace-root ql check sync should not fall back to the generic missing member package-name error line",
    );
    let error_line_index = normalized_stderr.find(&error_line).expect(
        "workspace-root ql check sync should include the missing member package-name error",
    );
    let package_note_index = normalized_stderr
        .find(&package_note)
        .expect("workspace-root ql check sync should include the package manifest note");
    let member_note_index = normalized_stderr
        .find(&member_note)
        .expect("workspace-root ql check sync should include the local member note");
    let rerun_hint_index = normalized_stderr
        .find(&rerun_hint)
        .expect("workspace-root ql check sync should include the rerun hint");
    assert!(
        error_line_index < package_note_index
            && package_note_index < member_note_index
            && member_note_index < rerun_hint_index,
        "expected sync missing member package-name context before rerun hint, got:\n{stderr}"
    );
    expect_stderr_contains(
        "project-check-workspace-sync-missing-member-package-name",
        "workspace-root ql check sync with missing member package name",
        &stderr,
        "`ql check --sync-interfaces` found 1 failing member(s)",
    )
    .expect("workspace-root ql check sync should summarize the single missing member package name");
    expect_stderr_not_contains(
        "project-check-workspace-sync-missing-member-package-name",
        "workspace-root ql check sync with missing member package name",
        &normalized_stderr,
        "note: first failing member manifest:",
    )
    .expect(
        "single sync missing member package-name failures should not repeat the manifest in the final summary",
    );
}

#[test]
fn check_workspace_root_preserves_missing_source_root_rerun_hint() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-workspace-missing-source-root");
    let app_root = temp.path().join("workspace").join("packages").join("app");
    let broken_root = temp
        .path()
        .join("workspace")
        .join("packages")
        .join("broken");
    let tool_root = temp.path().join("workspace").join("packages").join("tool");
    let app_source = app_root.join("src").join("lib.ql");
    let tool_source = tool_root.join("src").join("lib.ql");
    let workspace_manifest = temp.path().join("workspace");
    std::fs::create_dir_all(app_root.join("src")).expect("create app source directory");
    std::fs::create_dir_all(&broken_root).expect("create broken member directory");
    std::fs::create_dir_all(tool_root.join("src")).expect("create tool source directory");

    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/broken", "packages/tool"]
"#,
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace/packages/app/src/lib.ql",
        r#"
package demo.app

pub fn main() -> Int {
    return 1
}
"#,
    );
    temp.write(
        "workspace/packages/broken/qlang.toml",
        r#"
[package]
name = "broken"
"#,
    );
    temp.write(
        "workspace/packages/tool/qlang.toml",
        r#"
[package]
name = "tool"
"#,
    );
    temp.write(
        "workspace/packages/tool/src/lib.ql",
        r#"
package demo.tool

pub fn main() -> Int {
    return 2
}
"#,
    );

    let mut command = ql_command(&workspace_root);
    command.args(["check"]).arg(&workspace_manifest);
    let output = run_command_capture(
        &mut command,
        "`ql check` workspace root with missing source root member",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-check-workspace-missing-source-root",
        "workspace-root ql check with missing source root member",
        &output,
        1,
    )
    .expect("workspace-root ql check with missing source root member should fail");
    let normalized_stdout = stdout.replace('\\', "/");
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stdout_contains_all(
        "project-check-workspace-missing-source-root",
        &normalized_stdout,
        &[
            &format!(
                "ok: {}",
                app_source.display().to_string().replace('\\', "/")
            ),
            &format!(
                "ok: {}",
                tool_source.display().to_string().replace('\\', "/")
            ),
        ],
    )
    .expect("workspace-root ql check should continue checking later valid members");
    let broken_manifest = broken_root
        .join("qlang.toml")
        .display()
        .to_string()
        .replace('\\', "/");
    let broken_source_root = broken_root
        .join("src")
        .display()
        .to_string()
        .replace('\\', "/");
    let error_line =
        format!("error: `ql check` package source directory `{broken_source_root}` does not exist");
    let old_error_line =
        format!("error: package source directory `{broken_source_root}` does not exist");
    let package_note = format!("note: failing package manifest: {broken_manifest}");
    let member_note = format!("note: failing workspace member manifest: {broken_manifest}");
    let source_root_note = format!("note: failing package source root: {broken_source_root}");
    let rerun_hint =
        format!("hint: rerun `ql check {broken_manifest}` after fixing the package source root");
    expect_stderr_contains(
        "project-check-workspace-missing-source-root",
        "workspace-root ql check with missing source root member",
        &normalized_stderr,
        &error_line,
    )
    .expect("workspace-root ql check should preserve the command label for missing source roots");
    expect_stderr_not_contains(
        "project-check-workspace-missing-source-root",
        "workspace-root ql check with missing source root member",
        &normalized_stderr,
        &old_error_line,
    )
    .expect(
        "workspace-root ql check should not fall back to the generic missing source-root error",
    );
    expect_stderr_contains(
        "project-check-workspace-missing-source-root",
        "workspace-root ql check with missing source root member",
        &normalized_stderr,
        &package_note,
    )
    .expect("workspace-root ql check should point to the failing package manifest");
    expect_stderr_contains(
        "project-check-workspace-missing-source-root",
        "workspace-root ql check with missing source root member",
        &normalized_stderr,
        &member_note,
    )
    .expect("workspace-root ql check should keep the workspace member boundary visible");
    expect_stderr_contains(
        "project-check-workspace-missing-source-root",
        "workspace-root ql check with missing source root member",
        &normalized_stderr,
        &source_root_note,
    )
    .expect("workspace-root ql check should point to the missing source root");
    expect_stderr_contains(
        "project-check-workspace-missing-source-root",
        "workspace-root ql check with missing source root member",
        &normalized_stderr,
        &rerun_hint,
    )
    .expect("workspace-root ql check should suggest rerunning the failing member manifest");
    let package_note_index = normalized_stderr
        .find(&package_note)
        .expect("workspace-root ql check should include the package note");
    let member_note_index = normalized_stderr
        .find(&member_note)
        .expect("workspace-root ql check should include the member note");
    let source_root_note_index = normalized_stderr
        .find(&source_root_note)
        .expect("workspace-root ql check should include the source-root note");
    let rerun_hint_index = normalized_stderr
        .find(&rerun_hint)
        .expect("workspace-root ql check should include the rerun hint");
    assert!(
        package_note_index < member_note_index
            && member_note_index < source_root_note_index
            && source_root_note_index < rerun_hint_index,
        "expected workspace missing-source-root context before hint, got:\n{stderr}"
    );
    expect_stderr_contains(
        "project-check-workspace-missing-source-root",
        "workspace-root ql check with missing source root member",
        &stderr,
        "`ql check` found 1 failing member(s)",
    )
    .expect("workspace-root ql check should summarize the single missing source-root member");
    expect_stderr_not_contains(
        "project-check-workspace-missing-source-root",
        "workspace-root ql check with missing source root member",
        &normalized_stderr,
        "note: first failing member manifest:",
    )
    .expect("single workspace missing source-root members should not repeat the manifest in the final summary");
}

#[test]
fn check_workspace_root_sync_preserves_missing_source_root_rerun_hint() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-workspace-sync-missing-source-root");
    let app_root = temp.path().join("workspace").join("packages").join("app");
    let broken_root = temp
        .path()
        .join("workspace")
        .join("packages")
        .join("broken");
    let tool_root = temp.path().join("workspace").join("packages").join("tool");
    let app_source = app_root.join("src").join("lib.ql");
    let tool_source = tool_root.join("src").join("lib.ql");
    let workspace_manifest = temp.path().join("workspace");
    std::fs::create_dir_all(app_root.join("src")).expect("create app source directory");
    std::fs::create_dir_all(&broken_root).expect("create broken member directory");
    std::fs::create_dir_all(tool_root.join("src")).expect("create tool source directory");

    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/broken", "packages/tool"]
"#,
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace/packages/app/src/lib.ql",
        r#"
package demo.app

pub fn main() -> Int {
    return 1
}
"#,
    );
    temp.write(
        "workspace/packages/broken/qlang.toml",
        r#"
[package]
name = "broken"
"#,
    );
    temp.write(
        "workspace/packages/tool/qlang.toml",
        r#"
[package]
name = "tool"
"#,
    );
    temp.write(
        "workspace/packages/tool/src/lib.ql",
        r#"
package demo.tool

pub fn main() -> Int {
    return 2
}
"#,
    );

    let mut command = ql_command(&workspace_root);
    command
        .args(["check", "--sync-interfaces"])
        .arg(&workspace_manifest);
    let output = run_command_capture(
        &mut command,
        "`ql check --sync-interfaces` workspace root with missing source root member",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-check-workspace-sync-missing-source-root",
        "workspace-root ql check sync with missing source root member",
        &output,
        1,
    )
    .expect("workspace-root ql check sync with missing source root member should fail");
    let normalized_stdout = stdout.replace('\\', "/");
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stdout_contains_all(
        "project-check-workspace-sync-missing-source-root",
        &normalized_stdout,
        &[
            &format!(
                "ok: {}",
                app_source.display().to_string().replace('\\', "/")
            ),
            &format!(
                "ok: {}",
                tool_source.display().to_string().replace('\\', "/")
            ),
        ],
    )
    .expect("workspace-root ql check sync should continue checking later valid members");
    let broken_manifest = broken_root
        .join("qlang.toml")
        .display()
        .to_string()
        .replace('\\', "/");
    let broken_source_root = broken_root
        .join("src")
        .display()
        .to_string()
        .replace('\\', "/");
    let error_line = format!(
        "error: `ql check --sync-interfaces` package source directory `{broken_source_root}` does not exist"
    );
    let old_error_line =
        format!("error: package source directory `{broken_source_root}` does not exist");
    let package_note = format!("note: failing package manifest: {broken_manifest}");
    let member_note = format!("note: failing workspace member manifest: {broken_manifest}");
    let source_root_note = format!("note: failing package source root: {broken_source_root}");
    let rerun_hint = format!(
        "hint: rerun `ql check --sync-interfaces {broken_manifest}` after fixing the package source root"
    );
    expect_stderr_contains(
        "project-check-workspace-sync-missing-source-root",
        "workspace-root ql check sync with missing source root member",
        &normalized_stderr,
        &error_line,
    )
    .expect(
        "workspace-root ql check sync should preserve the command label for missing source roots",
    );
    expect_stderr_not_contains(
        "project-check-workspace-sync-missing-source-root",
        "workspace-root ql check sync with missing source root member",
        &normalized_stderr,
        &old_error_line,
    )
    .expect("workspace-root ql check sync should not fall back to the generic missing source-root error");
    expect_stderr_contains(
        "project-check-workspace-sync-missing-source-root",
        "workspace-root ql check sync with missing source root member",
        &normalized_stderr,
        &package_note,
    )
    .expect("workspace-root ql check sync should point to the failing package manifest");
    expect_stderr_contains(
        "project-check-workspace-sync-missing-source-root",
        "workspace-root ql check sync with missing source root member",
        &normalized_stderr,
        &member_note,
    )
    .expect("workspace-root ql check sync should keep the workspace member boundary visible");
    expect_stderr_contains(
        "project-check-workspace-sync-missing-source-root",
        "workspace-root ql check sync with missing source root member",
        &normalized_stderr,
        &source_root_note,
    )
    .expect("workspace-root ql check sync should point to the missing source root");
    expect_stderr_contains(
        "project-check-workspace-sync-missing-source-root",
        "workspace-root ql check sync with missing source root member",
        &normalized_stderr,
        &rerun_hint,
    )
    .expect("workspace-root ql check sync should suggest rerunning the failing member manifest");
    let package_note_index = normalized_stderr
        .find(&package_note)
        .expect("workspace-root ql check sync should include the package note");
    let member_note_index = normalized_stderr
        .find(&member_note)
        .expect("workspace-root ql check sync should include the member note");
    let source_root_note_index = normalized_stderr
        .find(&source_root_note)
        .expect("workspace-root ql check sync should include the source-root note");
    let rerun_hint_index = normalized_stderr
        .find(&rerun_hint)
        .expect("workspace-root ql check sync should include the rerun hint");
    assert!(
        package_note_index < member_note_index
            && member_note_index < source_root_note_index
            && source_root_note_index < rerun_hint_index,
        "expected workspace sync missing-source-root context before hint, got:\n{stderr}"
    );
    expect_stderr_contains(
        "project-check-workspace-sync-missing-source-root",
        "workspace-root ql check sync with missing source root member",
        &stderr,
        "`ql check --sync-interfaces` found 1 failing member(s)",
    )
    .expect("workspace-root ql check sync should summarize the single missing source-root member");
    expect_stderr_not_contains(
        "project-check-workspace-sync-missing-source-root",
        "workspace-root ql check sync with missing source root member",
        &normalized_stderr,
        "note: first failing member manifest:",
    )
    .expect("single workspace sync missing source-root members should not repeat the manifest in the final summary");
}

#[test]
fn check_workspace_root_preserves_empty_source_root_rerun_hint() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-workspace-empty-source-root");
    let app_root = temp.path().join("workspace").join("packages").join("app");
    let broken_root = temp
        .path()
        .join("workspace")
        .join("packages")
        .join("broken");
    let tool_root = temp.path().join("workspace").join("packages").join("tool");
    let app_source = app_root.join("src").join("lib.ql");
    let tool_source = tool_root.join("src").join("lib.ql");
    let workspace_manifest = temp.path().join("workspace");
    std::fs::create_dir_all(app_root.join("src")).expect("create app source directory");
    std::fs::create_dir_all(broken_root.join("src")).expect("create broken source directory");
    std::fs::create_dir_all(tool_root.join("src")).expect("create tool source directory");

    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/broken", "packages/tool"]
"#,
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace/packages/app/src/lib.ql",
        r#"
package demo.app

pub fn main() -> Int {
    return 1
}
"#,
    );
    temp.write(
        "workspace/packages/broken/qlang.toml",
        r#"
[package]
name = "broken"
"#,
    );
    temp.write(
        "workspace/packages/tool/qlang.toml",
        r#"
[package]
name = "tool"
"#,
    );
    temp.write(
        "workspace/packages/tool/src/lib.ql",
        r#"
package demo.tool

pub fn main() -> Int {
    return 2
}
"#,
    );

    let mut command = ql_command(&workspace_root);
    command.args(["check"]).arg(&workspace_manifest);
    let output = run_command_capture(
        &mut command,
        "`ql check` workspace root with empty source root member",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-check-workspace-empty-source-root",
        "workspace-root ql check with empty source root member",
        &output,
        1,
    )
    .expect("workspace-root ql check with empty source root member should fail");
    let normalized_stdout = stdout.replace('\\', "/");
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stdout_contains_all(
        "project-check-workspace-empty-source-root",
        &normalized_stdout,
        &[
            &format!(
                "ok: {}",
                app_source.display().to_string().replace('\\', "/")
            ),
            &format!(
                "ok: {}",
                tool_source.display().to_string().replace('\\', "/")
            ),
        ],
    )
    .expect("workspace-root ql check should continue checking later valid members");
    let broken_manifest = broken_root
        .join("qlang.toml")
        .display()
        .to_string()
        .replace('\\', "/");
    let broken_source_root = broken_root
        .join("src")
        .display()
        .to_string()
        .replace('\\', "/");
    let error_line = format!("error: `ql check` no `.ql` files found under `{broken_source_root}`");
    let old_error_line = format!("error: no `.ql` files found under `{broken_source_root}`");
    let package_note = format!("note: failing package manifest: {broken_manifest}");
    let member_note = format!("note: failing workspace member manifest: {broken_manifest}");
    let source_root_note = format!("note: failing package source root: {broken_source_root}");
    let rerun_hint =
        format!("hint: rerun `ql check {broken_manifest}` after adding package source files");
    expect_stderr_contains(
        "project-check-workspace-empty-source-root",
        "workspace-root ql check with empty source root member",
        &normalized_stderr,
        &error_line,
    )
    .expect("workspace-root ql check should preserve the command label for empty source roots");
    expect_stderr_not_contains(
        "project-check-workspace-empty-source-root",
        "workspace-root ql check with empty source root member",
        &normalized_stderr,
        &old_error_line,
    )
    .expect("workspace-root ql check should not fall back to the generic empty source-root error");
    expect_stderr_contains(
        "project-check-workspace-empty-source-root",
        "workspace-root ql check with empty source root member",
        &normalized_stderr,
        &package_note,
    )
    .expect("workspace-root ql check should point to the failing package manifest");
    expect_stderr_contains(
        "project-check-workspace-empty-source-root",
        "workspace-root ql check with empty source root member",
        &normalized_stderr,
        &member_note,
    )
    .expect("workspace-root ql check should keep the workspace member boundary visible");
    expect_stderr_contains(
        "project-check-workspace-empty-source-root",
        "workspace-root ql check with empty source root member",
        &normalized_stderr,
        &source_root_note,
    )
    .expect("workspace-root ql check should point to the empty source root");
    expect_stderr_contains(
        "project-check-workspace-empty-source-root",
        "workspace-root ql check with empty source root member",
        &normalized_stderr,
        &rerun_hint,
    )
    .expect("workspace-root ql check should suggest rerunning the failing member manifest");
    let package_note_index = normalized_stderr
        .find(&package_note)
        .expect("workspace-root ql check should include the package note");
    let member_note_index = normalized_stderr
        .find(&member_note)
        .expect("workspace-root ql check should include the member note");
    let source_root_note_index = normalized_stderr
        .find(&source_root_note)
        .expect("workspace-root ql check should include the source-root note");
    let rerun_hint_index = normalized_stderr
        .find(&rerun_hint)
        .expect("workspace-root ql check should include the rerun hint");
    assert!(
        package_note_index < member_note_index
            && member_note_index < source_root_note_index
            && source_root_note_index < rerun_hint_index,
        "expected workspace empty-source-root context before hint, got:\n{stderr}"
    );
    expect_stderr_contains(
        "project-check-workspace-empty-source-root",
        "workspace-root ql check with empty source root member",
        &stderr,
        "`ql check` found 1 failing member(s)",
    )
    .expect("workspace-root ql check should summarize the single empty source-root member");
    expect_stderr_not_contains(
        "project-check-workspace-empty-source-root",
        "workspace-root ql check with empty source root member",
        &normalized_stderr,
        "note: first failing member manifest:",
    )
    .expect("single workspace empty source-root members should not repeat the manifest in the final summary");
}

#[test]
fn check_workspace_root_sync_preserves_empty_source_root_rerun_hint() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-workspace-sync-empty-source-root");
    let app_root = temp.path().join("workspace").join("packages").join("app");
    let broken_root = temp
        .path()
        .join("workspace")
        .join("packages")
        .join("broken");
    let tool_root = temp.path().join("workspace").join("packages").join("tool");
    let app_source = app_root.join("src").join("lib.ql");
    let tool_source = tool_root.join("src").join("lib.ql");
    let workspace_manifest = temp.path().join("workspace");
    std::fs::create_dir_all(app_root.join("src")).expect("create app source directory");
    std::fs::create_dir_all(broken_root.join("src")).expect("create broken source directory");
    std::fs::create_dir_all(tool_root.join("src")).expect("create tool source directory");

    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/broken", "packages/tool"]
"#,
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace/packages/app/src/lib.ql",
        r#"
package demo.app

pub fn main() -> Int {
    return 1
}
"#,
    );
    temp.write(
        "workspace/packages/broken/qlang.toml",
        r#"
[package]
name = "broken"
"#,
    );
    temp.write(
        "workspace/packages/tool/qlang.toml",
        r#"
[package]
name = "tool"
"#,
    );
    temp.write(
        "workspace/packages/tool/src/lib.ql",
        r#"
package demo.tool

pub fn main() -> Int {
    return 2
}
"#,
    );

    let mut command = ql_command(&workspace_root);
    command
        .args(["check", "--sync-interfaces"])
        .arg(&workspace_manifest);
    let output = run_command_capture(
        &mut command,
        "`ql check --sync-interfaces` workspace root with empty source root member",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-check-workspace-sync-empty-source-root",
        "workspace-root ql check sync with empty source root member",
        &output,
        1,
    )
    .expect("workspace-root ql check sync with empty source root member should fail");
    let normalized_stdout = stdout.replace('\\', "/");
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stdout_contains_all(
        "project-check-workspace-sync-empty-source-root",
        &normalized_stdout,
        &[
            &format!(
                "ok: {}",
                app_source.display().to_string().replace('\\', "/")
            ),
            &format!(
                "ok: {}",
                tool_source.display().to_string().replace('\\', "/")
            ),
        ],
    )
    .expect("workspace-root ql check sync should continue checking later valid members");
    let broken_manifest = broken_root
        .join("qlang.toml")
        .display()
        .to_string()
        .replace('\\', "/");
    let broken_source_root = broken_root
        .join("src")
        .display()
        .to_string()
        .replace('\\', "/");
    let error_line = format!(
        "error: `ql check --sync-interfaces` no `.ql` files found under `{broken_source_root}`"
    );
    let old_error_line = format!("error: no `.ql` files found under `{broken_source_root}`");
    let package_note = format!("note: failing package manifest: {broken_manifest}");
    let member_note = format!("note: failing workspace member manifest: {broken_manifest}");
    let source_root_note = format!("note: failing package source root: {broken_source_root}");
    let rerun_hint = format!(
        "hint: rerun `ql check --sync-interfaces {broken_manifest}` after adding package source files"
    );
    expect_stderr_contains(
        "project-check-workspace-sync-empty-source-root",
        "workspace-root ql check sync with empty source root member",
        &normalized_stderr,
        &error_line,
    )
    .expect(
        "workspace-root ql check sync should preserve the command label for empty source roots",
    );
    expect_stderr_not_contains(
        "project-check-workspace-sync-empty-source-root",
        "workspace-root ql check sync with empty source root member",
        &normalized_stderr,
        &old_error_line,
    )
    .expect(
        "workspace-root ql check sync should not fall back to the generic empty source-root error",
    );
    expect_stderr_contains(
        "project-check-workspace-sync-empty-source-root",
        "workspace-root ql check sync with empty source root member",
        &normalized_stderr,
        &package_note,
    )
    .expect("workspace-root ql check sync should point to the failing package manifest");
    expect_stderr_contains(
        "project-check-workspace-sync-empty-source-root",
        "workspace-root ql check sync with empty source root member",
        &normalized_stderr,
        &member_note,
    )
    .expect("workspace-root ql check sync should keep the workspace member boundary visible");
    expect_stderr_contains(
        "project-check-workspace-sync-empty-source-root",
        "workspace-root ql check sync with empty source root member",
        &normalized_stderr,
        &source_root_note,
    )
    .expect("workspace-root ql check sync should point to the empty source root");
    expect_stderr_contains(
        "project-check-workspace-sync-empty-source-root",
        "workspace-root ql check sync with empty source root member",
        &normalized_stderr,
        &rerun_hint,
    )
    .expect("workspace-root ql check sync should suggest rerunning the failing member manifest");
    let package_note_index = normalized_stderr
        .find(&package_note)
        .expect("workspace-root ql check sync should include the package note");
    let member_note_index = normalized_stderr
        .find(&member_note)
        .expect("workspace-root ql check sync should include the member note");
    let source_root_note_index = normalized_stderr
        .find(&source_root_note)
        .expect("workspace-root ql check sync should include the source-root note");
    let rerun_hint_index = normalized_stderr
        .find(&rerun_hint)
        .expect("workspace-root ql check sync should include the rerun hint");
    assert!(
        package_note_index < member_note_index
            && member_note_index < source_root_note_index
            && source_root_note_index < rerun_hint_index,
        "expected workspace sync empty-source-root context before hint, got:\n{stderr}"
    );
    expect_stderr_contains(
        "project-check-workspace-sync-empty-source-root",
        "workspace-root ql check sync with empty source root member",
        &stderr,
        "`ql check --sync-interfaces` found 1 failing member(s)",
    )
    .expect("workspace-root ql check sync should summarize the single empty source-root member");
    expect_stderr_not_contains(
        "project-check-workspace-sync-empty-source-root",
        "workspace-root ql check sync with empty source root member",
        &normalized_stderr,
        "note: first failing member manifest:",
    )
    .expect("single workspace sync empty source-root members should not repeat the manifest in the final summary");
}

#[test]
fn check_workspace_root_preserves_source_diagnostic_rerun_hint() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-workspace-source-diagnostics");
    let app_root = temp.path().join("workspace").join("packages").join("app");
    let broken_root = temp
        .path()
        .join("workspace")
        .join("packages")
        .join("broken");
    let tool_root = temp.path().join("workspace").join("packages").join("tool");
    let app_source = app_root.join("src").join("lib.ql");
    let broken_source = broken_root.join("src").join("lib.ql");
    let tool_source = tool_root.join("src").join("lib.ql");
    let workspace_manifest = temp.path().join("workspace");
    std::fs::create_dir_all(app_root.join("src")).expect("create app source directory");
    std::fs::create_dir_all(broken_root.join("src")).expect("create broken source directory");
    std::fs::create_dir_all(tool_root.join("src")).expect("create tool source directory");

    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/broken", "packages/tool"]
"#,
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace/packages/app/src/lib.ql",
        r#"
package demo.app

pub fn main() -> Int {
    return 1
}
"#,
    );
    temp.write(
        "workspace/packages/broken/qlang.toml",
        r#"
[package]
name = "broken"
"#,
    );
    temp.write(
        "workspace/packages/broken/src/lib.ql",
        r#"
package demo.broken

pub fn main( -> Int {
    return 2
}
"#,
    );
    temp.write(
        "workspace/packages/tool/qlang.toml",
        r#"
[package]
name = "tool"
"#,
    );
    temp.write(
        "workspace/packages/tool/src/lib.ql",
        r#"
package demo.tool

pub fn main() -> Int {
    return 3
}
"#,
    );

    let mut command = ql_command(&workspace_root);
    command.args(["check"]).arg(&workspace_manifest);
    let output = run_command_capture(
        &mut command,
        "`ql check` workspace root with source diagnostics",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-check-workspace-source-diagnostics",
        "workspace-root ql check with source diagnostics",
        &output,
        1,
    )
    .expect("workspace-root ql check with source diagnostics should fail");
    let normalized_stdout = stdout.replace('\\', "/");
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stdout_contains_all(
        "project-check-workspace-source-diagnostics",
        &normalized_stdout,
        &[
            &format!(
                "ok: {}",
                app_source.display().to_string().replace('\\', "/")
            ),
            &format!(
                "ok: {}",
                tool_source.display().to_string().replace('\\', "/")
            ),
        ],
    )
    .expect("workspace-root ql check should continue checking later valid members");
    let broken_manifest = broken_root
        .join("qlang.toml")
        .display()
        .to_string()
        .replace('\\', "/");
    let package_note = format!("note: failing package manifest: {broken_manifest}");
    let member_note = format!("note: failing workspace member manifest: {broken_manifest}");
    let rerun_hint =
        format!("hint: rerun `ql check {broken_manifest}` after fixing the package sources");
    let broken_source_line = broken_source.display().to_string().replace('\\', "/");
    expect_stderr_contains(
        "project-check-workspace-source-diagnostics",
        "workspace-root ql check with source diagnostics",
        &normalized_stderr,
        &broken_source_line,
    )
    .expect("workspace-root ql check should surface the broken source path");
    expect_stderr_contains(
        "project-check-workspace-source-diagnostics",
        "workspace-root ql check with source diagnostics",
        &normalized_stderr,
        &member_note,
    )
    .expect("workspace-root ql check should point source diagnostics at the member manifest");
    expect_stderr_contains(
        "project-check-workspace-source-diagnostics",
        "workspace-root ql check with source diagnostics",
        &normalized_stderr,
        &rerun_hint,
    )
    .expect(
        "workspace-root ql check should suggest rerunning the broken member after fixing sources",
    );
    let broken_source_index = normalized_stderr
        .find(&broken_source_line)
        .expect("workspace-root ql check should include the broken source path");
    let package_note_index = normalized_stderr
        .find(&package_note)
        .expect("workspace-root ql check should include the package manifest note");
    let member_note_index = normalized_stderr
        .find(&member_note)
        .expect("workspace-root ql check should include the member note");
    let rerun_hint_index = normalized_stderr
        .find(&rerun_hint)
        .expect("workspace-root ql check should include the rerun hint");
    assert!(
        broken_source_index < package_note_index
            && package_note_index < member_note_index
            && member_note_index < rerun_hint_index,
        "expected workspace source diagnostics before member rerun hint, got:\n{stderr}"
    );
    expect_stderr_contains(
        "project-check-workspace-source-diagnostics",
        "workspace-root ql check with source diagnostics",
        &stderr,
        "`ql check` found 1 failing member(s)",
    )
    .expect("workspace-root ql check should summarize the single source-diagnostic member");
    expect_stderr_not_contains(
        "project-check-workspace-source-diagnostics",
        "workspace-root ql check with source diagnostics",
        &normalized_stderr,
        "note: first failing member manifest:",
    )
    .expect("single source-diagnostic workspace members should not repeat the manifest in the final summary");
}

#[test]
fn check_workspace_root_supports_json_source_diagnostics() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-workspace-json-source-diagnostics");
    let dep_root = temp.path().join("workspace").join("dep");
    let app_root = temp.path().join("workspace").join("packages").join("app");
    let broken_root = temp
        .path()
        .join("workspace")
        .join("packages")
        .join("broken");
    let tool_root = temp.path().join("workspace").join("packages").join("tool");
    let app_source = app_root.join("src").join("lib.ql");
    let broken_source = broken_root.join("src").join("lib.ql");
    let tool_source = tool_root.join("src").join("lib.ql");
    let workspace_manifest = temp.path().join("workspace");
    std::fs::create_dir_all(dep_root.join("src")).expect("create dependency source directory");
    std::fs::create_dir_all(app_root.join("src")).expect("create app source directory");
    std::fs::create_dir_all(broken_root.join("src")).expect("create broken source directory");
    std::fs::create_dir_all(tool_root.join("src")).expect("create tool source directory");

    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/broken", "packages/tool"]
"#,
    );
    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub fn exported() -> Int
"#,
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../../dep"]
"#,
    );
    let broken_manifest = temp.write(
        "workspace/packages/broken/qlang.toml",
        r#"
[package]
name = "broken"
"#,
    );
    temp.write(
        "workspace/packages/tool/qlang.toml",
        r#"
[package]
name = "tool"
"#,
    );
    temp.write(
        "workspace/packages/app/src/lib.ql",
        r#"
package demo.app

pub fn main() -> Int {
    return 1
}
"#,
    );
    temp.write(
        "workspace/packages/broken/src/lib.ql",
        r#"
package demo.broken

pub fn main() -> Int {
    return "oops"
}
"#,
    );
    temp.write(
        "workspace/packages/tool/src/lib.ql",
        r#"
package demo.tool

pub fn main() -> Int {
    return 3
}
"#,
    );

    let mut command = ql_command(&workspace_root);
    command.args(["check", "--json"]).arg(&workspace_manifest);
    let output = run_command_capture(
        &mut command,
        "`ql check --json` workspace root with source diagnostics",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-check-workspace-json-source-diagnostics",
        "workspace-root ql check json with source diagnostics",
        &output,
        1,
    )
    .expect("workspace-root ql check json with source diagnostics should fail");
    expect_empty_stderr(
        "project-check-workspace-json-source-diagnostics",
        "workspace-root ql check json with source diagnostics",
        &stderr,
    )
    .expect("workspace-root ql check json with source diagnostics should keep stderr empty");

    let normalized_stdout = stdout.replace('\\', "/");
    expect_stdout_contains_all(
        "project-check-workspace-json-source-diagnostics",
        &normalized_stdout,
        &[
            "\"schema\": \"ql.check.v1\"",
            "\"scope\": \"workspace\"",
            "\"status\": \"diagnostics\"",
            &format!(
                "\"project_manifest_path\": \"{}\"",
                workspace_manifest
                    .join("qlang.toml")
                    .display()
                    .to_string()
                    .replace('\\', "/")
            ),
            &format!(
                "\"{}\"",
                app_source.display().to_string().replace('\\', "/")
            ),
            &format!(
                "\"{}\"",
                tool_source.display().to_string().replace('\\', "/")
            ),
            &format!(
                "\"{}\"",
                broken_source.display().to_string().replace('\\', "/")
            ),
            &format!(
                "\"{}\"",
                broken_manifest.display().to_string().replace('\\', "/")
            ),
            "\"message\": \"return value has type mismatch: expected `Int`, found `String`\"",
        ],
    )
    .expect("workspace-root ql check json should report healthy files and structured diagnostics");
}

#[test]
fn check_workspace_root_sync_preserves_source_diagnostic_rerun_hint() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-workspace-sync-source-diagnostics");
    let app_root = temp.path().join("workspace").join("packages").join("app");
    let broken_root = temp
        .path()
        .join("workspace")
        .join("packages")
        .join("broken");
    let tool_root = temp.path().join("workspace").join("packages").join("tool");
    let app_source = app_root.join("src").join("lib.ql");
    let broken_source = broken_root.join("src").join("lib.ql");
    let tool_source = tool_root.join("src").join("lib.ql");
    let workspace_manifest = temp.path().join("workspace");
    std::fs::create_dir_all(app_root.join("src")).expect("create app source directory");
    std::fs::create_dir_all(broken_root.join("src")).expect("create broken source directory");
    std::fs::create_dir_all(tool_root.join("src")).expect("create tool source directory");

    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/broken", "packages/tool"]
"#,
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace/packages/app/src/lib.ql",
        r#"
package demo.app

pub fn main() -> Int {
    return 1
}
"#,
    );
    temp.write(
        "workspace/packages/broken/qlang.toml",
        r#"
[package]
name = "broken"
"#,
    );
    temp.write(
        "workspace/packages/broken/src/lib.ql",
        r#"
package demo.broken

pub fn main( -> Int {
    return 2
}
"#,
    );
    temp.write(
        "workspace/packages/tool/qlang.toml",
        r#"
[package]
name = "tool"
"#,
    );
    temp.write(
        "workspace/packages/tool/src/lib.ql",
        r#"
package demo.tool

pub fn main() -> Int {
    return 3
}
"#,
    );

    let mut command = ql_command(&workspace_root);
    command
        .args(["check", "--sync-interfaces"])
        .arg(&workspace_manifest);
    let output = run_command_capture(
        &mut command,
        "`ql check --sync-interfaces` workspace root with source diagnostics",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-check-workspace-sync-source-diagnostics",
        "workspace-root ql check sync with source diagnostics",
        &output,
        1,
    )
    .expect("workspace-root ql check sync with source diagnostics should fail");
    let normalized_stdout = stdout.replace('\\', "/");
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stdout_contains_all(
        "project-check-workspace-sync-source-diagnostics",
        &normalized_stdout,
        &[
            &format!(
                "ok: {}",
                app_source.display().to_string().replace('\\', "/")
            ),
            &format!(
                "ok: {}",
                tool_source.display().to_string().replace('\\', "/")
            ),
        ],
    )
    .expect("workspace-root ql check sync should continue checking later valid members");
    let broken_manifest = broken_root
        .join("qlang.toml")
        .display()
        .to_string()
        .replace('\\', "/");
    let package_note = format!("note: failing package manifest: {broken_manifest}");
    let member_note = format!("note: failing workspace member manifest: {broken_manifest}");
    let rerun_hint = format!(
        "hint: rerun `ql check --sync-interfaces {broken_manifest}` after fixing the package sources"
    );
    let broken_source_line = broken_source.display().to_string().replace('\\', "/");
    expect_stderr_contains(
        "project-check-workspace-sync-source-diagnostics",
        "workspace-root ql check sync with source diagnostics",
        &normalized_stderr,
        &broken_source_line,
    )
    .expect("workspace-root ql check sync should surface the broken source path");
    expect_stderr_contains(
        "project-check-workspace-sync-source-diagnostics",
        "workspace-root ql check sync with source diagnostics",
        &normalized_stderr,
        &member_note,
    )
    .expect("workspace-root ql check sync should point source diagnostics at the member manifest");
    expect_stderr_contains(
        "project-check-workspace-sync-source-diagnostics",
        "workspace-root ql check sync with source diagnostics",
        &normalized_stderr,
        &rerun_hint,
    )
    .expect("workspace-root ql check sync should suggest rerunning the broken member after fixing sources");
    let broken_source_index = normalized_stderr
        .find(&broken_source_line)
        .expect("workspace-root ql check sync should include the broken source path");
    let package_note_index = normalized_stderr
        .find(&package_note)
        .expect("workspace-root ql check sync should include the package manifest note");
    let member_note_index = normalized_stderr
        .find(&member_note)
        .expect("workspace-root ql check sync should include the member note");
    let rerun_hint_index = normalized_stderr
        .find(&rerun_hint)
        .expect("workspace-root ql check sync should include the rerun hint");
    assert!(
        broken_source_index < package_note_index
            && package_note_index < member_note_index
            && member_note_index < rerun_hint_index,
        "expected workspace sync source diagnostics before member rerun hint, got:\n{stderr}"
    );
    expect_stderr_contains(
        "project-check-workspace-sync-source-diagnostics",
        "workspace-root ql check sync with source diagnostics",
        &stderr,
        "`ql check --sync-interfaces` found 1 failing member(s)",
    )
    .expect("workspace-root ql check sync should summarize the single source-diagnostic member");
    expect_stderr_not_contains(
        "project-check-workspace-sync-source-diagnostics",
        "workspace-root ql check sync with source diagnostics",
        &normalized_stderr,
        "note: first failing member manifest:",
    )
    .expect("single sync source-diagnostic workspace members should not repeat the manifest in the final summary");
}

#[test]
fn check_workspace_root_preserves_reference_failure_rerun_hint() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-workspace-reference-failure");
    let app_root = temp.path().join("workspace").join("packages").join("app");
    let broken_root = temp
        .path()
        .join("workspace")
        .join("packages")
        .join("broken");
    let tool_root = temp.path().join("workspace").join("packages").join("tool");
    let broken_ref_root = temp.path().join("workspace").join("broken_ref");
    let app_source = app_root.join("src").join("lib.ql");
    let tool_source = tool_root.join("src").join("lib.ql");
    let workspace_manifest = temp.path().join("workspace");
    std::fs::create_dir_all(app_root.join("src")).expect("create app source directory");
    std::fs::create_dir_all(broken_root.join("src")).expect("create broken source directory");
    std::fs::create_dir_all(tool_root.join("src")).expect("create tool source directory");
    std::fs::create_dir_all(&broken_ref_root).expect("create broken reference directory");

    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/broken", "packages/tool"]
"#,
    );
    temp.write(
        "workspace/broken_ref/qlang.toml",
        r#"
[package
name = "broken_ref"
"#,
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace/packages/app/src/lib.ql",
        r#"
package demo.app

pub fn main() -> Int {
    return 1
}
"#,
    );
    temp.write(
        "workspace/packages/broken/qlang.toml",
        r#"
[package]
name = "broken"

[references]
packages = ["../../broken_ref"]
"#,
    );
    temp.write(
        "workspace/packages/broken/src/lib.ql",
        r#"
package demo.broken

pub fn main() -> Int {
    return 2
}
"#,
    );
    temp.write(
        "workspace/packages/tool/qlang.toml",
        r#"
[package]
name = "tool"
"#,
    );
    temp.write(
        "workspace/packages/tool/src/lib.ql",
        r#"
package demo.tool

pub fn main() -> Int {
    return 3
}
"#,
    );

    let mut command = ql_command(&workspace_root);
    command.args(["check"]).arg(&workspace_manifest);
    let output = run_command_capture(
        &mut command,
        "`ql check` workspace root with reference failure",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-check-workspace-reference-failure",
        "workspace-root ql check with reference failure",
        &output,
        1,
    )
    .expect("workspace-root ql check with reference failure should fail");
    let normalized_stdout = stdout.replace('\\', "/");
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stdout_contains_all(
        "project-check-workspace-reference-failure",
        &normalized_stdout,
        &[
            &format!(
                "ok: {}",
                app_source.display().to_string().replace('\\', "/")
            ),
            &format!(
                "ok: {}",
                tool_source.display().to_string().replace('\\', "/")
            ),
        ],
    )
    .expect("workspace-root ql check should continue checking later valid members");
    let broken_manifest = broken_root
        .join("qlang.toml")
        .display()
        .to_string()
        .replace('\\', "/");
    let broken_ref_manifest = broken_ref_root
        .join("qlang.toml")
        .display()
        .to_string()
        .replace('\\', "/");
    let reference_note = format!("note: failing reference manifest: {broken_ref_manifest}");
    let package_note = format!("note: failing package manifest: {broken_manifest}");
    let member_note = format!("note: failing workspace member manifest: {broken_manifest}");
    let rerun_hint = format!(
        "hint: rerun `ql check {broken_manifest}` after fixing the referenced package or reference manifest"
    );
    let error_line = "error: `ql check` failed to load referenced package `../../broken_ref`";
    let old_error_line = "error: failed to load referenced package `../../broken_ref`";
    expect_stderr_contains(
        "project-check-workspace-reference-failure",
        "workspace-root ql check with reference failure",
        &normalized_stderr,
        error_line,
    )
    .expect("workspace-root ql check should preserve the ql check command label");
    expect_stderr_not_contains(
        "project-check-workspace-reference-failure",
        "workspace-root ql check with reference failure",
        &normalized_stderr,
        old_error_line,
    )
    .expect("workspace-root ql check should not fall back to the unlabeled reference error");
    expect_stderr_contains(
        "project-check-workspace-reference-failure",
        "workspace-root ql check with reference failure",
        &normalized_stderr,
        &reference_note,
    )
    .expect("workspace-root ql check should point to the failing reference manifest");
    expect_stderr_contains(
        "project-check-workspace-reference-failure",
        "workspace-root ql check with reference failure",
        &normalized_stderr,
        &member_note,
    )
    .expect(
        "workspace-root ql check should point the reference failure back to the member manifest",
    );
    expect_stderr_contains(
        "project-check-workspace-reference-failure",
        "workspace-root ql check with reference failure",
        &normalized_stderr,
        &rerun_hint,
    )
    .expect("workspace-root ql check should suggest rerunning the failing member after fixing references");
    let reference_note_index = normalized_stderr
        .find(&reference_note)
        .expect("workspace-root ql check should include the reference note");
    let package_note_index = normalized_stderr
        .find(&package_note)
        .expect("workspace-root ql check should include the package manifest note");
    let member_note_index = normalized_stderr
        .find(&member_note)
        .expect("workspace-root ql check should include the member note");
    let rerun_hint_index = normalized_stderr
        .find(&rerun_hint)
        .expect("workspace-root ql check should include the rerun hint");
    assert!(
        reference_note_index < package_note_index
            && package_note_index < member_note_index
            && member_note_index < rerun_hint_index,
        "expected workspace reference failure context before member rerun hint, got:\n{stderr}"
    );
    expect_stderr_contains(
        "project-check-workspace-reference-failure",
        "workspace-root ql check with reference failure",
        &stderr,
        "`ql check` found 1 failing member(s)",
    )
    .expect("workspace-root ql check should summarize the single reference-failing member");
    expect_stderr_not_contains(
        "project-check-workspace-reference-failure",
        "workspace-root ql check with reference failure",
        &normalized_stderr,
        "note: first failing member manifest:",
    )
    .expect("single reference-failing workspace members should not repeat the manifest in the final summary");
}

#[test]
fn check_workspace_root_sync_preserves_reference_failure_rerun_hint() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-workspace-sync-reference-failure");
    let app_root = temp.path().join("workspace").join("packages").join("app");
    let broken_root = temp
        .path()
        .join("workspace")
        .join("packages")
        .join("broken");
    let tool_root = temp.path().join("workspace").join("packages").join("tool");
    let broken_ref_root = temp.path().join("workspace").join("broken_ref");
    let app_source = app_root.join("src").join("lib.ql");
    let tool_source = tool_root.join("src").join("lib.ql");
    let workspace_manifest = temp.path().join("workspace");
    std::fs::create_dir_all(app_root.join("src")).expect("create app source directory");
    std::fs::create_dir_all(broken_root.join("src")).expect("create broken source directory");
    std::fs::create_dir_all(tool_root.join("src")).expect("create tool source directory");
    std::fs::create_dir_all(&broken_ref_root).expect("create broken reference directory");

    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/broken", "packages/tool"]
"#,
    );
    temp.write(
        "workspace/broken_ref/qlang.toml",
        r#"
[package
name = "broken_ref"
"#,
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace/packages/app/src/lib.ql",
        r#"
package demo.app

pub fn main() -> Int {
    return 1
}
"#,
    );
    temp.write(
        "workspace/packages/broken/qlang.toml",
        r#"
[package]
name = "broken"

[references]
packages = ["../../broken_ref"]
"#,
    );
    temp.write(
        "workspace/packages/broken/src/lib.ql",
        r#"
package demo.broken

pub fn main() -> Int {
    return 2
}
"#,
    );
    temp.write(
        "workspace/packages/tool/qlang.toml",
        r#"
[package]
name = "tool"
"#,
    );
    temp.write(
        "workspace/packages/tool/src/lib.ql",
        r#"
package demo.tool

pub fn main() -> Int {
    return 3
}
"#,
    );

    let mut command = ql_command(&workspace_root);
    command
        .args(["check", "--sync-interfaces"])
        .arg(&workspace_manifest);
    let output = run_command_capture(
        &mut command,
        "`ql check --sync-interfaces` workspace root with reference failure",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-check-workspace-sync-reference-failure",
        "workspace-root ql check sync with reference failure",
        &output,
        1,
    )
    .expect("workspace-root ql check sync with reference failure should fail");
    let normalized_stdout = stdout.replace('\\', "/");
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stdout_contains_all(
        "project-check-workspace-sync-reference-failure",
        &normalized_stdout,
        &[
            &format!(
                "ok: {}",
                app_source.display().to_string().replace('\\', "/")
            ),
            &format!(
                "ok: {}",
                tool_source.display().to_string().replace('\\', "/")
            ),
        ],
    )
    .expect("workspace-root ql check sync should continue checking later valid members");
    let broken_manifest = broken_root
        .join("qlang.toml")
        .display()
        .to_string()
        .replace('\\', "/");
    let broken_ref_manifest = broken_ref_root
        .join("qlang.toml")
        .display()
        .to_string()
        .replace('\\', "/");
    let reference_note = format!("note: failing reference manifest: {broken_ref_manifest}");
    let package_note = format!("note: failing package manifest: {broken_manifest}");
    let member_note = format!("note: failing workspace member manifest: {broken_manifest}");
    let rerun_hint = format!(
        "hint: rerun `ql check --sync-interfaces {broken_manifest}` after fixing the referenced package or reference manifest"
    );
    let error_line =
        "error: `ql check --sync-interfaces` failed to load referenced package `../../broken_ref`";
    let old_error_line = "error: failed to load referenced package `../../broken_ref`";
    expect_stderr_contains(
        "project-check-workspace-sync-reference-failure",
        "workspace-root ql check sync with reference failure",
        &normalized_stderr,
        error_line,
    )
    .expect("workspace-root ql check sync should preserve the ql check sync command label");
    expect_stderr_not_contains(
        "project-check-workspace-sync-reference-failure",
        "workspace-root ql check sync with reference failure",
        &normalized_stderr,
        old_error_line,
    )
    .expect("workspace-root ql check sync should not fall back to the unlabeled reference error");
    expect_stderr_contains(
        "project-check-workspace-sync-reference-failure",
        "workspace-root ql check sync with reference failure",
        &normalized_stderr,
        &reference_note,
    )
    .expect("workspace-root ql check sync should point to the failing reference manifest");
    expect_stderr_contains(
        "project-check-workspace-sync-reference-failure",
        "workspace-root ql check sync with reference failure",
        &normalized_stderr,
        &member_note,
    )
    .expect("workspace-root ql check sync should point the reference failure back to the member manifest");
    expect_stderr_contains(
        "project-check-workspace-sync-reference-failure",
        "workspace-root ql check sync with reference failure",
        &normalized_stderr,
        &rerun_hint,
    )
    .expect("workspace-root ql check sync should suggest rerunning the failing member after fixing references");
    let reference_note_index = normalized_stderr
        .find(&reference_note)
        .expect("workspace-root ql check sync should include the reference note");
    let package_note_index = normalized_stderr
        .find(&package_note)
        .expect("workspace-root ql check sync should include the package manifest note");
    let member_note_index = normalized_stderr
        .find(&member_note)
        .expect("workspace-root ql check sync should include the member note");
    let rerun_hint_index = normalized_stderr
        .find(&rerun_hint)
        .expect("workspace-root ql check sync should include the rerun hint");
    assert!(
        reference_note_index < package_note_index
            && package_note_index < member_note_index
            && member_note_index < rerun_hint_index,
        "expected workspace sync reference failure context before member rerun hint, got:\n{stderr}"
    );
    expect_stderr_contains(
        "project-check-workspace-sync-reference-failure",
        "workspace-root ql check sync with reference failure",
        &stderr,
        "`ql check --sync-interfaces` found 1 failing member(s)",
    )
    .expect("workspace-root ql check sync should summarize the single reference-failing member");
    expect_stderr_not_contains(
        "project-check-workspace-sync-reference-failure",
        "workspace-root ql check sync with reference failure",
        &normalized_stderr,
        "note: first failing member manifest:",
    )
    .expect("single sync reference-failing workspace members should not repeat the manifest in the final summary");
}

#[test]
fn check_workspace_root_reports_all_failing_members() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-workspace-failures");
    let good_dep_root = temp.path().join("workspace").join("deps").join("good");
    let missing_dep_root = temp.path().join("workspace").join("deps").join("missing");
    let good_root = temp.path().join("workspace").join("packages").join("good");
    let missing_root = temp
        .path()
        .join("workspace")
        .join("packages")
        .join("missing");
    let broken_root = temp
        .path()
        .join("workspace")
        .join("packages")
        .join("broken");
    let good_source = good_root.join("src").join("lib.ql");
    let workspace_manifest = temp.path().join("workspace");
    std::fs::create_dir_all(good_dep_root.join("src"))
        .expect("create good dependency source directory");
    std::fs::create_dir_all(missing_dep_root.join("src"))
        .expect("create missing dependency source directory");
    std::fs::create_dir_all(good_root.join("src")).expect("create good package source directory");
    std::fs::create_dir_all(missing_root.join("src"))
        .expect("create missing package source directory");
    std::fs::create_dir_all(broken_root.join("src"))
        .expect("create broken package source directory");

    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/good", "packages/missing", "packages/broken"]
"#,
    );
    temp.write(
        "workspace/deps/good/qlang.toml",
        r#"
[package]
name = "good_dep"
"#,
    );
    temp.write(
        "workspace/deps/good/good_dep.qi",
        r#"
// qlang interface v1
// package: good_dep

// source: src/lib.ql
package demo.good_dep

pub fn exported() -> Int
"#,
    );
    temp.write(
        "workspace/deps/missing/qlang.toml",
        r#"
[package]
name = "missing_dep"
"#,
    );
    temp.write(
        "workspace/packages/good/qlang.toml",
        r#"
[package]
name = "good"

[references]
packages = ["../../deps/good"]
"#,
    );
    temp.write(
        "workspace/packages/good/src/lib.ql",
        r#"
package demo.good

pub fn main() -> Int {
    return 1
}
"#,
    );
    temp.write(
        "workspace/packages/missing/qlang.toml",
        r#"
[package]
name = "missing"

[references]
packages = ["../../deps/missing"]
"#,
    );
    temp.write(
        "workspace/packages/missing/src/lib.ql",
        r#"
package demo.missing

pub fn main() -> Int {
    return 2
}
"#,
    );
    temp.write(
        "workspace/packages/broken/qlang.toml",
        r#"
[package]
name = "broken"
"#,
    );
    temp.write(
        "workspace/packages/broken/src/lib.ql",
        r#"
package demo.broken

pub fn main( -> Int {
    return 3
}
"#,
    );

    let mut command = ql_command(&workspace_root);
    command.args(["check"]).arg(&workspace_manifest);
    let output = run_command_capture(
        &mut command,
        "`ql check` workspace root with multiple failing members",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-check-workspace-failures",
        "workspace-root ql check with multiple failing members",
        &output,
        1,
    )
    .expect("workspace-root ql check with multiple failing members should fail");
    let normalized_stdout = stdout.replace('\\', "/");
    let normalized_good_source = good_source.display().to_string().replace('\\', "/");
    expect_stdout_contains_all(
        "project-check-workspace-failures",
        &normalized_stdout,
        &[
            &format!("ok: {normalized_good_source}"),
            "loaded interface: ",
            "good_dep.qi",
        ],
    )
    .expect("workspace-root ql check should still report successful members before the summary");
    expect_stderr_contains(
        "project-check-workspace-failures",
        "workspace-root ql check with multiple failing members",
        &stderr,
        "error: `ql check` referenced package `missing_dep` is missing interface artifact",
    )
    .expect("workspace-root ql check should preserve the ql check command label for missing dependency interfaces");
    expect_stderr_not_contains(
        "project-check-workspace-failures",
        "workspace-root ql check with multiple failing members",
        &stderr,
        "error: referenced package `missing_dep` is missing interface artifact",
    )
    .expect("workspace-root ql check should not fall back to the unlabeled artifact error");
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stderr_contains(
        "project-check-workspace-failures",
        "workspace-root ql check with multiple failing members",
        &normalized_stderr,
        &format!(
            "note: failing workspace member manifest: {}",
            missing_root
                .join("qlang.toml")
                .display()
                .to_string()
                .replace('\\', "/")
        ),
    )
    .expect("workspace-root ql check should point missing-reference failures at the member manifest immediately");
    expect_stderr_contains(
        "project-check-workspace-failures",
        "workspace-root ql check with multiple failing members",
        &normalized_stderr,
        "packages/broken/src/lib.ql",
    )
    .expect("workspace-root ql check should continue and surface later source diagnostics");
    expect_stderr_contains(
        "project-check-workspace-failures",
        "workspace-root ql check with multiple failing members",
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
    .expect(
        "workspace-root ql check should point source failures at the member manifest immediately",
    );
    expect_stderr_contains(
        "project-check-workspace-failures",
        "workspace-root ql check with multiple failing members",
        &stderr,
        "`ql check` found 2 failing member(s)",
    )
    .expect("workspace-root ql check should summarize all failing members");
    expect_stderr_contains(
        "project-check-workspace-failures",
        "workspace-root ql check with multiple failing members",
        &normalized_stderr,
        &format!(
            "note: first failing member manifest: {}",
            missing_root
                .join("qlang.toml")
                .display()
                .to_string()
                .replace('\\', "/")
        ),
    )
    .expect("workspace-root ql check should point to the first failing member manifest");
}

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
