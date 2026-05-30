mod support;

use std::path::PathBuf;

use support::{
    TempDir, expect_empty_stderr, expect_exit_code, expect_snapshot_matches,
    expect_stderr_contains, expect_stdout_contains_all, expect_success, ql_command,
    run_command_capture, workspace_root,
};

struct WorkspaceCheckPackageSelectorProject {
    temp: TempDir,
    workspace_manifest: PathBuf,
    app_root: PathBuf,
    app_source: PathBuf,
    tool_source: PathBuf,
    dep_interface: PathBuf,
}

fn write_workspace_check_package_selector_project(
    prefix: &str,
) -> WorkspaceCheckPackageSelectorProject {
    let temp = TempDir::new(prefix);
    let dep_root = temp.path().join("workspace").join("dep");
    let app_root = temp.path().join("workspace").join("packages").join("app");
    let tool_root = temp.path().join("workspace").join("packages").join("tool");
    let workspace_manifest = temp.path().join("workspace").join("qlang.toml");
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
    let dep_interface = temp.write(
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

    WorkspaceCheckPackageSelectorProject {
        temp,
        workspace_manifest,
        app_root,
        app_source,
        tool_source,
        dep_interface,
    }
}

fn expected_workspace_check_package_selector_json(
    fixture: &WorkspaceCheckPackageSelectorProject,
) -> String {
    format!(
        "{{\n  \"checked_files\": [\n    \"{}\"\n  ],\n  \"diagnostic_files\": [],\n  \"failing_manifests\": [],\n  \"loaded_interfaces\": [\n    \"{}\"\n  ],\n  \"project_manifest_path\": \"{}\",\n  \"schema\": \"ql.check.v1\",\n  \"scope\": \"workspace\",\n  \"status\": \"ok\",\n  \"sync_interfaces\": false,\n  \"written_interfaces\": []\n}}\n",
        fixture.app_source.display().to_string().replace('\\', "/"),
        fixture
            .dep_interface
            .display()
            .to_string()
            .replace('\\', "/"),
        fixture
            .workspace_manifest
            .display()
            .to_string()
            .replace('\\', "/"),
    )
}

fn expect_workspace_check_missing_package_selector_error(
    snapshot_name: &str,
    label: &str,
    fixture: &WorkspaceCheckPackageSelectorProject,
    stdout: &str,
    stderr: &str,
) {
    assert!(
        stdout.trim().is_empty(),
        "expected {label} stdout to stay empty, got:\n{stdout}"
    );
    let normalized_stderr = stderr.replace('\\', "/");
    let manifest_display = fixture
        .workspace_manifest
        .display()
        .to_string()
        .replace('\\', "/");
    expect_stderr_contains(
        snapshot_name,
        label,
        &normalized_stderr,
        &format!("error: `ql check` package selector matched no workspace members under `{manifest_display}`"),
    )
    .expect("ql check package selector should surface the missing-package error");
    expect_stderr_contains(
        snapshot_name,
        label,
        &normalized_stderr,
        "note: selector: package `missing`",
    )
    .expect("ql check package selector should include the selector note");
}

#[test]
fn check_workspace_member_source_supports_package_selectors() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-workspace-member-source-selector");
    let app_root = temp.path().join("workspace").join("packages").join("app");
    let tool_root = temp.path().join("workspace").join("packages").join("tool");
    let app_source = app_root.join("src").join("lib.ql");
    let tool_source = tool_root.join("src").join("lib.ql");
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
        .args(["check"])
        .arg(&app_source)
        .args(["--package", "tool"]);
    let output = run_command_capture(
        &mut command,
        "`ql check` workspace member source package selector",
    );
    let (stdout, stderr) = expect_success(
        "project-check-workspace-member-source-selector",
        "workspace member source ql check package selector",
        &output,
    )
    .expect("workspace member source ql check package selector should succeed");
    let normalized_stdout = stdout.replace('\\', "/");
    expect_stdout_contains_all(
        "project-check-workspace-member-source-selector",
        &normalized_stdout,
        &[&format!(
            "ok: {}",
            tool_source.display().to_string().replace('\\', "/")
        )],
    )
    .expect(
        "workspace member source ql check package selector should resolve the enclosing workspace",
    );
    assert!(
        !normalized_stdout.contains(&app_source.display().to_string().replace('\\', "/")),
        "workspace member source ql check package selector should skip the unselected member, got:\n{normalized_stdout}"
    );
    assert!(
        stderr.trim().is_empty(),
        "expected workspace member source ql check package selector stderr to stay empty, got:\n{stderr}"
    );
}

#[test]
fn check_workspace_member_directory_supports_package_selectors() {
    let workspace_root = workspace_root();
    let fixture = write_workspace_check_package_selector_project(
        "ql-project-check-workspace-member-directory-selector",
    );

    let mut command = ql_command(&workspace_root);
    command
        .args(["check"])
        .arg(&fixture.app_root)
        .args(["--package", "app"]);
    let output = run_command_capture(
        &mut command,
        "`ql check` workspace member directory package selector",
    );
    let (stdout, stderr) = expect_success(
        "project-check-workspace-member-directory-selector",
        "workspace member directory ql check package selector",
        &output,
    )
    .expect("workspace member directory ql check package selector should succeed");
    let normalized_stdout = stdout.replace('\\', "/");
    expect_stdout_contains_all(
        "project-check-workspace-member-directory-selector",
        &normalized_stdout,
        &[
            &format!(
                "ok: {}",
                fixture.app_source.display().to_string().replace('\\', "/")
            ),
            "loaded interface: ",
            "dep.qi",
        ],
    )
    .expect(
        "workspace member directory ql check package selector should resolve the enclosing workspace",
    );
    assert!(
        !normalized_stdout.contains(&fixture.tool_source.display().to_string().replace('\\', "/")),
        "workspace member directory ql check package selector should skip the unselected member, got:\n{normalized_stdout}"
    );
    assert!(
        stderr.trim().is_empty(),
        "expected workspace member directory ql check package selector stderr to stay empty, got:\n{stderr}"
    );
}

#[test]
fn check_workspace_root_supports_package_selectors() {
    let workspace_root = workspace_root();
    let fixture = write_workspace_check_package_selector_project(
        "ql-project-check-workspace-package-selector",
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["check"])
        .arg(&fixture.workspace_manifest)
        .args(["--package", "app"]);
    let output = run_command_capture(&mut command, "`ql check` workspace root package selector");
    let (stdout, stderr) = expect_success(
        "project-check-workspace-package-selector",
        "workspace-root ql check package selector",
        &output,
    )
    .expect("workspace-root ql check package selector should succeed");
    let normalized_stdout = stdout.replace('\\', "/");
    expect_stdout_contains_all(
        "project-check-workspace-package-selector",
        &normalized_stdout,
        &[
            &format!(
                "ok: {}",
                fixture.app_source.display().to_string().replace('\\', "/")
            ),
            "loaded interface: ",
            "dep.qi",
        ],
    )
    .expect("workspace-root ql check package selector should report the selected member");
    assert!(
        !normalized_stdout.contains(&fixture.tool_source.display().to_string().replace('\\', "/")),
        "workspace-root ql check package selector should skip unselected members, got:\n{normalized_stdout}"
    );
    assert!(
        stderr.trim().is_empty(),
        "expected workspace-root ql check package selector stderr to stay empty, got:\n{stderr}"
    );
}

#[test]
fn check_workspace_root_package_selector_supports_json_output() {
    let workspace_root = workspace_root();
    let fixture = write_workspace_check_package_selector_project(
        "ql-project-check-workspace-package-selector-json",
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["check", "--json"])
        .arg(&fixture.workspace_manifest)
        .args(["--package", "app"]);
    let output = run_command_capture(
        &mut command,
        "`ql check --json` workspace root package selector",
    );
    let (stdout, stderr) = expect_success(
        "project-check-workspace-package-selector-json",
        "workspace-root ql check package selector json",
        &output,
    )
    .expect("workspace-root ql check package selector json should succeed");
    expect_empty_stderr(
        "project-check-workspace-package-selector-json",
        "workspace-root ql check package selector json",
        &stderr,
    )
    .expect("workspace-root ql check package selector json should not print stderr");

    let expected = expected_workspace_check_package_selector_json(&fixture);
    let normalized_stdout = stdout.replace('\\', "/");
    expect_snapshot_matches(
        "project-check-workspace-package-selector-json",
        "workspace package selector check json stdout",
        &expected,
        &normalized_stdout,
    )
    .expect("workspace-root ql check package selector json should match the stable contract");
    assert!(
        !normalized_stdout.contains(&fixture.tool_source.display().to_string().replace('\\', "/")),
        "workspace-root ql check package selector json should skip unselected members, got:\n{normalized_stdout}"
    );
}

#[test]
fn check_workspace_member_source_package_selector_supports_json_output() {
    let workspace_root = workspace_root();
    let fixture = write_workspace_check_package_selector_project(
        "ql-project-check-workspace-member-source-package-selector-json",
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["check", "--json"])
        .arg(&fixture.app_source)
        .args(["--package", "app"]);
    let output = run_command_capture(
        &mut command,
        "`ql check --json` workspace member source package selector",
    );
    let (stdout, stderr) = expect_success(
        "project-check-workspace-member-source-package-selector-json",
        "workspace member source ql check package selector json",
        &output,
    )
    .expect("workspace member source ql check package selector json should succeed");
    expect_empty_stderr(
        "project-check-workspace-member-source-package-selector-json",
        "workspace member source ql check package selector json",
        &stderr,
    )
    .expect("workspace member source ql check package selector json should not print stderr");

    let expected = expected_workspace_check_package_selector_json(&fixture);
    let normalized_stdout = stdout.replace('\\', "/");
    expect_snapshot_matches(
        "project-check-workspace-member-source-package-selector-json",
        "workspace member source package selector check json stdout",
        &expected,
        &normalized_stdout,
    )
    .expect(
        "workspace member source ql check package selector json should match the stable contract",
    );
    assert!(
        !normalized_stdout.contains(&fixture.tool_source.display().to_string().replace('\\', "/")),
        "workspace member source ql check package selector json should skip unselected members, got:\n{normalized_stdout}"
    );
}

#[test]
fn check_workspace_member_directory_package_selector_supports_json_output() {
    let workspace_root = workspace_root();
    let fixture = write_workspace_check_package_selector_project(
        "ql-project-check-workspace-member-directory-package-selector-json",
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["check", "--json"])
        .arg(&fixture.app_root)
        .args(["--package", "app"]);
    let output = run_command_capture(
        &mut command,
        "`ql check --json` workspace member directory package selector",
    );
    let (stdout, stderr) = expect_success(
        "project-check-workspace-member-directory-package-selector-json",
        "workspace member directory ql check package selector json",
        &output,
    )
    .expect("workspace member directory ql check package selector json should succeed");
    expect_empty_stderr(
        "project-check-workspace-member-directory-package-selector-json",
        "workspace member directory ql check package selector json",
        &stderr,
    )
    .expect("workspace member directory ql check package selector json should not print stderr");

    let expected = expected_workspace_check_package_selector_json(&fixture);
    let normalized_stdout = stdout.replace('\\', "/");
    expect_snapshot_matches(
        "project-check-workspace-member-directory-package-selector-json",
        "workspace member directory package selector check json stdout",
        &expected,
        &normalized_stdout,
    )
    .expect("workspace member directory ql check package selector json should match the stable contract");
    assert!(
        !normalized_stdout.contains(&fixture.tool_source.display().to_string().replace('\\', "/")),
        "workspace member directory ql check package selector json should skip unselected members, got:\n{normalized_stdout}"
    );
}

#[test]
fn check_direct_source_file_rejects_package_selectors_without_workspace_context() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-direct-source-package-selector");
    let source_path = temp.write(
        "standalone.ql",
        r#"
package demo

pub fn main() -> Int {
    return 1
}
"#,
    );

    let mut command = ql_command(&workspace_root);
    command
        .args(["check"])
        .arg(&source_path)
        .args(["--package", "app"]);
    let output = run_command_capture(&mut command, "`ql check` direct source package selector");
    let (stdout, stderr) = expect_exit_code(
        "project-check-direct-source-package-selector",
        "direct source ql check package selector",
        &output,
        1,
    )
    .expect("direct source ql check package selector should fail");
    assert!(
        stdout.trim().is_empty(),
        "expected direct source ql check package selector stdout to stay empty, got:\n{stdout}"
    );
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stderr_contains(
        "project-check-direct-source-package-selector",
        "direct source ql check package selector",
        &normalized_stderr,
        "error: `ql check` package selectors require a workspace path",
    )
    .expect("direct source ql check package selector should require a workspace path");
    expect_stderr_contains(
        "project-check-direct-source-package-selector",
        "direct source ql check package selector",
        &normalized_stderr,
        "note: selector: package `app`",
    )
    .expect("direct source ql check package selector should report the selector");
}

#[test]
fn check_package_path_rejects_package_selectors_without_workspace_context() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-package-path-package-selector");
    let package_root = temp.path().join("app");
    std::fs::create_dir_all(package_root.join("src")).expect("create package source directory");

    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "app/src/lib.ql",
        r#"
package demo.app

pub fn main() -> Int {
    return 1
}
"#,
    );

    let mut command = ql_command(&workspace_root);
    command
        .args(["check"])
        .arg(&package_root)
        .args(["--package", "app"]);
    let output = run_command_capture(&mut command, "`ql check` package path package selector");
    let (stdout, stderr) = expect_exit_code(
        "project-check-package-path-package-selector",
        "package path ql check package selector",
        &output,
        1,
    )
    .expect("package path ql check package selector should fail outside a workspace");
    assert!(
        stdout.trim().is_empty(),
        "expected package path ql check package selector stdout to stay empty, got:\n{stdout}"
    );
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stderr_contains(
        "project-check-package-path-package-selector",
        "package path ql check package selector",
        &normalized_stderr,
        "error: `ql check` package selectors require a workspace path",
    )
    .expect("package path ql check package selector should stay workspace-only");
    expect_stderr_contains(
        "project-check-package-path-package-selector",
        "package path ql check package selector",
        &normalized_stderr,
        "note: selector: package `app`",
    )
    .expect("package path ql check package selector should report the selector");
}

#[test]
fn check_workspace_root_package_selector_reports_missing_packages() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-workspace-package-selector-missing");
    let app_root = temp.path().join("workspace").join("packages").join("app");
    let workspace_manifest = temp.path().join("workspace").join("qlang.toml");
    std::fs::create_dir_all(app_root.join("src")).expect("create app source directory");

    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app"]
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

    let mut command = ql_command(&workspace_root);
    command
        .args(["check"])
        .arg(&workspace_manifest)
        .args(["--package", "missing"]);
    let output = run_command_capture(
        &mut command,
        "`ql check` workspace root package selector missing package",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-check-workspace-package-selector-missing",
        "workspace-root ql check package selector missing package",
        &output,
        1,
    )
    .expect("workspace-root ql check package selector should fail when the package is missing");
    assert!(
        stdout.trim().is_empty(),
        "expected workspace-root ql check package selector missing stdout to stay empty, got:\n{stdout}"
    );
    let normalized_stderr = stderr.replace('\\', "/");
    let manifest_display = workspace_manifest.display().to_string().replace('\\', "/");
    expect_stderr_contains(
        "project-check-workspace-package-selector-missing",
        "workspace-root ql check package selector missing package",
        &normalized_stderr,
        &format!(
            "error: `ql check` package selector matched no workspace members under `{manifest_display}`"
        ),
    )
    .expect("workspace-root ql check package selector should surface the missing-package error");
    expect_stderr_contains(
        "project-check-workspace-package-selector-missing",
        "workspace-root ql check package selector missing package",
        &normalized_stderr,
        "note: selector: package `missing`",
    )
    .expect("workspace-root ql check package selector should include the selector note");
}

#[test]
fn check_workspace_member_source_package_selector_reports_missing_packages() {
    let workspace_root = workspace_root();
    let fixture = write_workspace_check_package_selector_project(
        "ql-project-check-workspace-member-source-package-selector-missing",
    );

    let mut command = ql_command(&workspace_root);
    command
        .args(["check"])
        .arg(&fixture.app_source)
        .args(["--package", "missing"]);
    let output = run_command_capture(
        &mut command,
        "`ql check` workspace member source package selector missing package",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-check-workspace-member-source-package-selector-missing",
        "workspace member source ql check package selector missing package",
        &output,
        1,
    )
    .expect(
        "workspace member source ql check package selector should fail when the package is missing",
    );
    expect_workspace_check_missing_package_selector_error(
        "project-check-workspace-member-source-package-selector-missing",
        "workspace member source ql check package selector missing package",
        &fixture,
        &stdout,
        &stderr,
    );
}

#[test]
fn check_workspace_member_directory_package_selector_reports_missing_packages() {
    let workspace_root = workspace_root();
    let fixture = write_workspace_check_package_selector_project(
        "ql-project-check-workspace-member-directory-package-selector-missing",
    );

    let mut command = ql_command(&workspace_root);
    command
        .args(["check"])
        .arg(&fixture.app_root)
        .args(["--package", "missing"]);
    let output = run_command_capture(
        &mut command,
        "`ql check` workspace member directory package selector missing package",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-check-workspace-member-directory-package-selector-missing",
        "workspace member directory ql check package selector missing package",
        &output,
        1,
    )
    .expect("workspace member directory ql check package selector should fail when the package is missing");
    expect_workspace_check_missing_package_selector_error(
        "project-check-workspace-member-directory-package-selector-missing",
        "workspace member directory ql check package selector missing package",
        &fixture,
        &stdout,
        &stderr,
    );
}

#[test]
fn check_workspace_root_package_selector_reports_unresolved_workspace_member_metadata() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-workspace-package-selector-broken-member");
    let app_root = temp.path().join("workspace").join("packages").join("app");
    let broken_root = temp
        .path()
        .join("workspace")
        .join("packages")
        .join("broken");
    let workspace_manifest = temp.path().join("workspace").join("qlang.toml");
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
[package]
version = "0.1.0"
"#,
    );

    let mut command = ql_command(&workspace_root);
    command
        .args(["check"])
        .arg(&workspace_manifest)
        .args(["--package", "app"]);
    let output = run_command_capture(
        &mut command,
        "`ql check` workspace root package selector with broken member metadata",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-check-workspace-package-selector-broken-member",
        "workspace-root ql check package selector with broken member metadata",
        &output,
        1,
    )
    .expect("workspace-root ql check package selector should fail when another member metadata is unresolved");
    assert!(
        stdout.trim().is_empty(),
        "expected workspace-root ql check package selector with broken member metadata stdout to stay empty, got:\n{stdout}"
    );
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stderr_contains(
        "project-check-workspace-package-selector-broken-member",
        "workspace-root ql check package selector with broken member metadata",
        &normalized_stderr,
        "error: `ql check` failed to inspect workspace member `packages/broken`: manifest",
    )
    .expect("workspace-root ql check package selector should surface the broken member error");
    expect_stderr_contains(
        "project-check-workspace-package-selector-broken-member",
        "workspace-root ql check package selector with broken member metadata",
        &stderr,
        "does not declare `[package].name`",
    )
    .expect(
        "workspace-root ql check package selector should preserve the package-name failure detail",
    );
}
