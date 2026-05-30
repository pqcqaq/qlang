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
