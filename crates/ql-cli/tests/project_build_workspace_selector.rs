mod support;

use std::path::PathBuf;
use std::process::Stdio;

use serde_json::Value as JsonValue;
use support::{
    TempDir, assert_no_atomic_write_temp_files, assert_no_build_lock_directories,
    expect_empty_stderr, expect_empty_stdout, expect_exit_code, expect_file_exists,
    expect_stdout_contains_all, expect_success, ql_command, run_command_capture,
    static_library_output_path, workspace_root,
};

fn normalize_output_text(text: &str) -> String {
    text.replace("\r\n", "\n")
}

fn parse_json_output(case_name: &str, stdout: &str) -> JsonValue {
    serde_json::from_str(&normalize_output_text(stdout))
        .unwrap_or_else(|error| panic!("[{case_name}] parse json stdout: {error}\n{stdout}"))
}

struct WorkspaceBuildFixture {
    temp: TempDir,
    project_root: PathBuf,
    app_output: PathBuf,
    tool_output: PathBuf,
}

fn workspace_build_fixture(prefix: &str) -> WorkspaceBuildFixture {
    let temp = TempDir::new(prefix);
    let project_root = temp.path().join("workspace");
    std::fs::create_dir_all(project_root.join("packages/app/src"))
        .expect("create app package source tree");
    std::fs::create_dir_all(project_root.join("packages/tool/src"))
        .expect("create tool package source tree");

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
        "workspace/packages/tool/qlang.toml",
        r#"
[package]
name = "tool"
"#,
    );
    temp.write(
        "workspace/packages/app/src/lib.ql",
        "pub fn app_value() -> Int { return 1 }\n",
    );
    temp.write(
        "workspace/packages/tool/src/lib.ql",
        "pub fn tool_value() -> Int { return 2 }\n",
    );

    let app_output =
        static_library_output_path(&project_root.join("packages/app/target/ql/debug"), "lib");
    let tool_output =
        static_library_output_path(&project_root.join("packages/tool/target/ql/debug"), "lib");

    WorkspaceBuildFixture {
        temp,
        project_root,
        app_output,
        tool_output,
    }
}

struct WorkspacePackageRelativeTargetBuildFixture {
    temp: TempDir,
    project_root: PathBuf,
    workspace_manifest: PathBuf,
    app_manifest: PathBuf,
    app_admin_output: PathBuf,
    app_main_output: PathBuf,
    app_interface_output: PathBuf,
    tool_output: PathBuf,
}

fn workspace_package_relative_target_build_fixture(
    prefix: &str,
) -> WorkspacePackageRelativeTargetBuildFixture {
    let temp = TempDir::new(prefix);
    let project_root = temp.path().join("workspace");
    std::fs::create_dir_all(project_root.join("packages/app/src/bin"))
        .expect("create app package source tree");
    std::fs::create_dir_all(project_root.join("packages/tool/src"))
        .expect("create tool package source tree");

    let workspace_manifest = temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/tool"]
"#,
    );
    let app_manifest = temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"
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
        "workspace/packages/app/src/main.ql",
        "fn main() -> Int { return 1 }\n",
    );
    temp.write(
        "workspace/packages/app/src/bin/admin.ql",
        "fn main() -> Int { return 2 }\n",
    );
    temp.write(
        "workspace/packages/tool/src/main.ql",
        "fn main() -> Int { return 3 }\n",
    );

    let app_admin_output = project_root.join("packages/app/target/ql/debug/bin/admin.ll");
    let app_main_output = project_root.join("packages/app/target/ql/debug/main.ll");
    let app_interface_output = project_root.join("packages/app/app.qi");
    let tool_output = project_root.join("packages/tool/target/ql/debug/main.ll");

    WorkspacePackageRelativeTargetBuildFixture {
        temp,
        project_root,
        workspace_manifest,
        app_manifest,
        app_admin_output,
        app_main_output,
        app_interface_output,
        tool_output,
    }
}

struct WorkspaceDependencyClosureFixture {
    temp: TempDir,
    project_root: PathBuf,
    workspace_manifest: PathBuf,
    app_manifest: PathBuf,
    core_manifest: PathBuf,
    app_output: PathBuf,
    app_interface_output: PathBuf,
    core_output: PathBuf,
    tool_output: PathBuf,
}

fn workspace_dependency_closure_fixture(prefix: &str) -> WorkspaceDependencyClosureFixture {
    let temp = TempDir::new(prefix);
    let project_root = temp.path().join("workspace");
    std::fs::create_dir_all(project_root.join("packages/app/src"))
        .expect("create app package source tree");
    std::fs::create_dir_all(project_root.join("packages/core/src"))
        .expect("create core package source tree");
    std::fs::create_dir_all(project_root.join("packages/tool/src"))
        .expect("create tool package source tree");

    let workspace_manifest = temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/core", "packages/tool"]
"#,
    );
    let app_manifest = temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"

[dependencies]
core = "../core"
"#,
    );
    let core_manifest = temp.write(
        "workspace/packages/core/qlang.toml",
        r#"
[package]
name = "core"
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
        "pub fn app_value() -> Int { return 1 }\n",
    );
    temp.write(
        "workspace/packages/core/src/lib.ql",
        "pub fn core_value() -> Int { return 2 }\n",
    );
    temp.write(
        "workspace/packages/tool/src/lib.ql",
        "pub fn tool_value() -> Int { return 3 }\n",
    );

    let app_output =
        static_library_output_path(&project_root.join("packages/app/target/ql/debug"), "lib");
    let app_interface_output = project_root.join("packages/app/app.qi");
    let core_output =
        static_library_output_path(&project_root.join("packages/core/target/ql/debug"), "lib");
    let tool_output =
        static_library_output_path(&project_root.join("packages/tool/target/ql/debug"), "lib");

    WorkspaceDependencyClosureFixture {
        temp,
        project_root,
        workspace_manifest,
        app_manifest,
        core_manifest,
        app_output,
        app_interface_output,
        core_output,
        tool_output,
    }
}

fn run_workspace_build_success(
    fixture: &TempDir,
    project_root: &PathBuf,
    args: &[&str],
    command_name: &str,
    case_name: &str,
    description: &str,
) -> (String, String) {
    let workspace_root = workspace_root();
    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.path());
    command.args(["build"]).arg(project_root).args(args);
    let output = run_command_capture(&mut command, command_name);
    let (stdout, stderr) = expect_success(case_name, description, &output)
        .unwrap_or_else(|message| panic!("{description} should succeed: {message}"));
    expect_empty_stderr(case_name, description, &stderr)
        .unwrap_or_else(|message| panic!("{description} should not print stderr: {message}"));
    (stdout, stderr)
}

#[test]
fn build_workspace_path_builds_each_member_target() {
    let fixture = workspace_build_fixture("ql-project-build-workspace");

    let (stdout, _) = run_workspace_build_success(
        &fixture.temp,
        &fixture.project_root,
        &[],
        "`ql build` workspace path",
        "project-build-workspace",
        "workspace path build",
    );
    expect_stdout_contains_all(
        "project-build-workspace",
        &stdout.replace('\\', "/"),
        &[
            &format!("wrote staticlib: {}", fixture.app_output.display()).replace('\\', "/"),
            &format!("wrote staticlib: {}", fixture.tool_output.display()).replace('\\', "/"),
        ],
    )
    .expect("workspace path build should report each member artifact");

    expect_file_exists(
        "project-build-workspace",
        &fixture.app_output,
        "workspace app artifact",
        "workspace path build",
    )
    .expect("workspace path build should emit the app artifact");
    expect_file_exists(
        "project-build-workspace",
        &fixture.tool_output,
        "workspace tool artifact",
        "workspace path build",
    )
    .expect("workspace path build should emit the tool artifact");
}

#[test]
fn build_workspace_path_selects_requested_package_targets() {
    let fixture = workspace_build_fixture("ql-project-build-package-selector");

    let (stdout, _) = run_workspace_build_success(
        &fixture.temp,
        &fixture.project_root,
        &["--package", "app"],
        "`ql build --package` workspace path",
        "project-build-package-selector",
        "workspace package selector build",
    );
    expect_stdout_contains_all(
        "project-build-package-selector",
        &stdout.replace('\\', "/"),
        &[&format!("wrote staticlib: {}", fixture.app_output.display()).replace('\\', "/")],
    )
    .expect("workspace-path `ql build --package` should report only the selected package artifact");
    expect_file_exists(
        "project-build-package-selector",
        &fixture.app_output,
        "selected package artifact",
        "workspace package selector build",
    )
    .expect("workspace-path `ql build --package` should emit the selected package artifact");
    assert!(
        !fixture.tool_output.exists(),
        "workspace-path `ql build --package` should not build unselected package artifacts"
    );
}

#[test]
fn build_workspace_package_selector_accepts_package_relative_target_path() {
    let fixture =
        workspace_package_relative_target_build_fixture("ql-project-build-package-relative-target");

    let (stdout, _) = run_workspace_build_success(
        &fixture.temp,
        &fixture.project_root,
        &["--package", "app", "--target", "src/bin/admin.ql"],
        "`ql build --package --target` package-relative target",
        "project-build-package-relative-target",
        "workspace package-relative target selector build",
    );
    let normalized_stdout = stdout.replace('\\', "/");
    let admin_fragment =
        format!("wrote llvm-ir: {}", fixture.app_admin_output.display()).replace('\\', "/");
    expect_stdout_contains_all(
        "project-build-package-relative-target",
        &normalized_stdout,
        &[&admin_fragment],
    )
    .expect("workspace package-relative target selector build should report selected artifact");
    assert!(
        !normalized_stdout
            .contains(&format!("{}", fixture.app_main_output.display()).replace('\\', "/")),
        "workspace package-relative target selector build should not report unselected app main artifact"
    );
    assert!(
        !normalized_stdout
            .contains(&format!("{}", fixture.tool_output.display()).replace('\\', "/")),
        "workspace package-relative target selector build should not report unselected package artifact"
    );
    expect_file_exists(
        "project-build-package-relative-target",
        &fixture.app_admin_output,
        "selected package-relative target artifact",
        "workspace package-relative target selector build",
    )
    .expect("workspace package-relative target selector build should emit selected artifact");
    assert!(
        !fixture.app_main_output.exists(),
        "workspace package-relative target selector build should not build unselected app main"
    );
    assert!(
        !fixture.tool_output.exists(),
        "workspace package-relative target selector build should not build unselected package"
    );
}

#[test]
fn build_workspace_package_selector_json_reports_package_relative_target_path() {
    let fixture = workspace_package_relative_target_build_fixture(
        "ql-project-build-json-package-relative-target",
    );

    let (stdout, _) = run_workspace_build_success(
        &fixture.temp,
        &fixture.project_root,
        &["--package", "app", "--target", "src/bin/admin.ql", "--json"],
        "`ql build --package --target --json` package-relative target",
        "project-build-json-package-relative-target",
        "workspace package-relative target selector build json",
    );
    let json = parse_json_output("project-build-json-package-relative-target", &stdout);
    let expected = serde_json::json!({
        "schema": "ql.build.v1",
        "path": fixture.project_root.display().to_string().replace('\\', "/"),
        "scope": "project",
        "project_manifest_path": fixture.workspace_manifest.display().to_string().replace('\\', "/"),
        "requested_emit": "llvm-ir",
        "requested_profile": "debug",
        "profile_overridden": false,
        "emit_interface": false,
        "status": "ok",
        "built_targets": [
            {
                "manifest_path": fixture.app_manifest.display().to_string().replace('\\', "/"),
                "package_name": "app",
                "selected": true,
                "dependency_only": false,
                "kind": "bin",
                "path": "src/bin/admin.ql",
                "emit": "llvm-ir",
                "profile": "debug",
                "artifact_path": fixture.app_admin_output.display().to_string().replace('\\', "/"),
                "c_header_path": JsonValue::Null,
            }
        ],
        "interfaces": [
            {
                "manifest_path": fixture.app_manifest.display().to_string().replace('\\', "/"),
                "package_name": "app",
                "selected": true,
                "status": "wrote",
                "path": fixture.app_interface_output.display().to_string().replace('\\', "/"),
            }
        ],
        "failure": JsonValue::Null,
    });
    assert_eq!(
        json, expected,
        "workspace-path `ql build --package --target --json` should report the selected package-relative target"
    );
    expect_file_exists(
        "project-build-json-package-relative-target",
        &fixture.app_admin_output,
        "selected package-relative target artifact",
        "workspace package-relative target selector build json",
    )
    .expect("workspace package-relative target selector build json should emit selected artifact");
    expect_file_exists(
        "project-build-json-package-relative-target",
        &fixture.app_interface_output,
        "selected package interface",
        "workspace package-relative target selector build json",
    )
    .expect("workspace package-relative target selector build json should emit selected interface");
    assert!(
        !fixture.app_main_output.exists(),
        "workspace package-relative target selector build json should not build unselected app main"
    );
    assert!(
        !fixture.tool_output.exists(),
        "workspace package-relative target selector build json should not build unselected package"
    );
}

#[test]
fn build_workspace_package_selector_list_json_reports_package_relative_target_path() {
    let fixture = workspace_package_relative_target_build_fixture(
        "ql-project-build-list-json-package-relative-target",
    );

    let (stdout, _) = run_workspace_build_success(
        &fixture.temp,
        &fixture.project_root,
        &[
            "--list",
            "--json",
            "--package",
            "app",
            "--target",
            "src/bin/admin.ql",
        ],
        "`ql build --list --json --package --target` package-relative target",
        "project-build-list-json-package-relative-target",
        "workspace package-relative target selector build list json",
    );
    let json = parse_json_output("project-build-list-json-package-relative-target", &stdout);
    let expected = serde_json::json!({
        "schema": "ql.project.targets.v1",
        "members": [
            {
                "manifest_path": fixture.app_manifest.display().to_string().replace('\\', "/"),
                "package_name": "app",
                "targets": [
                    {
                        "kind": "bin",
                        "path": "src/bin/admin.ql",
                    }
                ],
            }
        ],
    });
    assert_eq!(
        json, expected,
        "workspace-path `ql build --list --json --package --target` should report the selected package-relative target"
    );
    assert!(
        !fixture.app_admin_output.exists(),
        "workspace package-relative target selector build list json should not build artifacts"
    );
}

#[test]
fn build_workspace_package_selector_list_json_reports_missing_package_relative_target_path() {
    let fixture = workspace_package_relative_target_build_fixture(
        "ql-project-build-list-json-missing-package-relative-target",
    );
    let workspace_root = workspace_root();
    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command.args(["build"]).arg(&fixture.project_root).args([
        "--list",
        "--json",
        "--package",
        "app",
        "--target",
        "src/bin/missing.ql",
    ]);
    let output = run_command_capture(
        &mut command,
        "`ql build --list --json --package --target` missing package-relative target",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-build-list-json-missing-package-relative-target",
        "workspace missing package-relative target selector build list json",
        &output,
        1,
    )
    .expect(
        "workspace-path `ql build --list --json --package app --target src/bin/missing.ql` should fail",
    );
    expect_empty_stderr(
        "project-build-list-json-missing-package-relative-target",
        "workspace missing package-relative target selector build list json",
        &stderr,
    )
    .expect(
        "workspace missing package-relative target selector build list json should stay on stdout",
    );

    let json = parse_json_output(
        "project-build-list-json-missing-package-relative-target",
        &stdout,
    );
    assert_eq!(json["schema"], "ql.project.targets.v1");
    assert_eq!(json["members"], serde_json::json!([]));
    assert_eq!(json["failure"]["kind"], "selection");
    let failure = &json["failure"]["selection_failure"];
    assert_eq!(failure["stage"], "target-selection");
    assert_eq!(
        failure["selector"],
        "package `app`, target `src/bin/missing.ql`"
    );
    assert_eq!(failure["target_count"], 3);
    assert!(
        failure["message"]
            .as_str()
            .expect("build list json selector miss should expose a message")
            .contains("target selector matched no build targets"),
        "missing package-relative target selector build list json should describe selector miss: {json}"
    );
    assert!(
        !fixture.app_admin_output.exists(),
        "workspace missing package-relative target selector build list json should not build artifacts"
    );
}

#[test]
fn build_workspace_package_selector_rejects_missing_package_relative_target_path() {
    let fixture = workspace_package_relative_target_build_fixture(
        "ql-project-build-missing-package-relative-target",
    );
    let workspace_root = workspace_root();
    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command.args(["build"]).arg(&fixture.project_root).args([
        "--package",
        "app",
        "--target",
        "src/bin/missing.ql",
    ]);
    let output = run_command_capture(
        &mut command,
        "`ql build --package --target` missing package-relative target",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-build-missing-package-relative-target",
        "workspace missing package-relative target selector build",
        &output,
        1,
    )
    .expect("workspace-path `ql build --package --target src/bin/missing.ql` should fail");
    expect_empty_stdout(
        "project-build-missing-package-relative-target",
        "workspace missing package-relative target selector build",
        &stdout,
    )
    .expect("workspace missing package-relative target selector build should not print stdout");
    assert!(
        stderr.contains("error: `ql build` target selector matched no build targets"),
        "missing package-relative target selector build should describe selector miss, got:\n{stderr}"
    );
    assert!(
        stderr.contains("note: selector: package `app`, target `src/bin/missing.ql`"),
        "missing package-relative target selector build should print selector note, got:\n{stderr}"
    );
    assert!(
        !fixture.app_admin_output.exists(),
        "workspace missing package-relative target selector build should not build artifacts"
    );
}

#[test]
fn build_workspace_package_selector_json_reports_missing_package_relative_target_path() {
    let fixture = workspace_package_relative_target_build_fixture(
        "ql-project-build-json-missing-package-relative-target",
    );
    let workspace_root = workspace_root();
    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command.args(["build"]).arg(&fixture.project_root).args([
        "--package",
        "app",
        "--target",
        "src/bin/missing.ql",
        "--json",
    ]);
    let output = run_command_capture(
        &mut command,
        "`ql build --package --target --json` missing package-relative target",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-build-json-missing-package-relative-target",
        "workspace missing package-relative target selector build json",
        &output,
        1,
    )
    .expect("workspace-path `ql build --package --target src/bin/missing.ql --json` should fail");
    expect_empty_stderr(
        "project-build-json-missing-package-relative-target",
        "workspace missing package-relative target selector build json",
        &stderr,
    )
    .expect(
        "workspace missing package-relative target selector build json should not print stderr",
    );

    let json = parse_json_output(
        "project-build-json-missing-package-relative-target",
        &stdout,
    );
    assert_eq!(json["schema"], "ql.build.v1");
    assert_eq!(json["status"], "failed");
    assert_eq!(json["built_targets"], serde_json::json!([]));
    assert_eq!(json["interfaces"], serde_json::json!([]));
    assert_eq!(json["failure"]["error_kind"], "selector");
    assert_eq!(json["failure"]["stage"], "target-selection");
    assert_eq!(
        json["failure"]["selector"],
        "package `app`, target `src/bin/missing.ql`"
    );
    assert_eq!(json["failure"]["target_count"], 0);
    assert!(
        json["failure"]["message"]
            .as_str()
            .expect("missing target json failure should expose message")
            .contains("target selector matched no build targets"),
        "missing package-relative target selector build json should describe selector miss: {json}"
    );
    assert!(
        !fixture.app_admin_output.exists(),
        "workspace missing package-relative target selector build json should not build artifacts"
    );
}

#[test]
fn build_workspace_package_selector_builds_local_dependency_packages_first() {
    let fixture =
        workspace_dependency_closure_fixture("ql-project-build-workspace-dependency-closure");

    let (stdout, _) = run_workspace_build_success(
        &fixture.temp,
        &fixture.project_root,
        &["--package", "app"],
        "`ql build --package` workspace dependency closure",
        "project-build-workspace-dependency-closure",
        "workspace package selector dependency closure build",
    );
    let normalized_stdout = stdout.replace('\\', "/");
    let core_fragment =
        format!("wrote staticlib: {}", fixture.core_output.display()).replace('\\', "/");
    let app_fragment =
        format!("wrote staticlib: {}", fixture.app_output.display()).replace('\\', "/");
    expect_stdout_contains_all(
        "project-build-workspace-dependency-closure",
        &normalized_stdout,
        &[&core_fragment, &app_fragment],
    )
    .expect(
        "workspace package selector dependency closure build should report dependency and root artifacts",
    );

    let core_index = normalized_stdout
        .find(&core_fragment)
        .expect("workspace dependency artifact should be present in stdout");
    let app_index = normalized_stdout
        .find(&app_fragment)
        .expect("workspace root artifact should be present in stdout");
    assert!(
        core_index < app_index,
        "workspace dependency package should be built before the selected root package, got:\n{stdout}"
    );

    expect_file_exists(
        "project-build-workspace-dependency-closure",
        &fixture.core_output,
        "workspace dependency artifact",
        "workspace package selector dependency closure build",
    )
    .expect(
        "workspace package selector dependency closure build should emit the dependency artifact",
    );
    expect_file_exists(
        "project-build-workspace-dependency-closure",
        &fixture.app_output,
        "workspace selected package artifact",
        "workspace package selector dependency closure build",
    )
    .expect(
        "workspace package selector dependency closure build should emit the selected package artifact",
    );
    assert!(
        !fixture.tool_output.exists(),
        "workspace package selector dependency closure build should not build unrelated package artifacts"
    );
}

#[test]
fn build_workspace_package_selector_json_reports_dependency_closure() {
    let fixture =
        workspace_dependency_closure_fixture("ql-project-build-workspace-dependency-closure-json");

    let (stdout, _) = run_workspace_build_success(
        &fixture.temp,
        &fixture.project_root,
        &["--package", "app", "--json"],
        "`ql build --package --json` workspace dependency closure",
        "project-build-workspace-dependency-closure-json",
        "workspace package selector dependency closure build json",
    );
    let json = parse_json_output("project-build-workspace-dependency-closure-json", &stdout);
    let expected = serde_json::json!({
        "schema": "ql.build.v1",
        "path": fixture.project_root.display().to_string().replace('\\', "/"),
        "scope": "project",
        "project_manifest_path": fixture.workspace_manifest.display().to_string().replace('\\', "/"),
        "requested_emit": "llvm-ir",
        "requested_profile": "debug",
        "profile_overridden": false,
        "emit_interface": false,
        "status": "ok",
        "built_targets": [
            {
                "manifest_path": fixture.core_manifest.display().to_string().replace('\\', "/"),
                "package_name": "core",
                "selected": false,
                "dependency_only": true,
                "kind": "lib",
                "path": "src/lib.ql",
                "emit": "staticlib",
                "profile": "debug",
                "artifact_path": fixture.core_output.display().to_string().replace('\\', "/"),
                "c_header_path": JsonValue::Null,
            },
            {
                "manifest_path": fixture.app_manifest.display().to_string().replace('\\', "/"),
                "package_name": "app",
                "selected": true,
                "dependency_only": false,
                "kind": "lib",
                "path": "src/lib.ql",
                "emit": "staticlib",
                "profile": "debug",
                "artifact_path": fixture.app_output.display().to_string().replace('\\', "/"),
                "c_header_path": JsonValue::Null,
            }
        ],
        "interfaces": [
            {
                "manifest_path": fixture.app_manifest.display().to_string().replace('\\', "/"),
                "package_name": "app",
                "selected": true,
                "status": "wrote",
                "path": fixture.app_interface_output.display().to_string().replace('\\', "/"),
            }
        ],
        "failure": JsonValue::Null,
    });
    assert_eq!(
        json, expected,
        "workspace-path `ql build --package --json` should report dependency closure in build order"
    );
    expect_file_exists(
        "project-build-workspace-dependency-closure-json",
        &fixture.core_output,
        "workspace dependency artifact",
        "workspace package selector dependency closure build json",
    )
    .expect(
        "workspace package selector dependency closure build json should emit dependency artifact",
    );
    expect_file_exists(
        "project-build-workspace-dependency-closure-json",
        &fixture.app_output,
        "workspace selected package artifact",
        "workspace package selector dependency closure build json",
    )
    .expect(
        "workspace package selector dependency closure build json should emit selected package artifact",
    );
    expect_file_exists(
        "project-build-workspace-dependency-closure-json",
        &fixture.app_interface_output,
        "workspace selected package interface",
        "workspace package selector dependency closure build json",
    )
    .expect("workspace package selector dependency closure build json should emit selected package interface");
    assert!(
        !fixture.tool_output.exists(),
        "workspace package selector dependency closure build json should not build unrelated package artifacts"
    );
}

#[test]
fn build_workspace_package_selector_json_allows_concurrent_dependency_closure_builds() {
    let workspace_root = workspace_root();
    let fixture = workspace_dependency_closure_fixture(
        "ql-project-build-workspace-concurrent-dependency-closure-json",
    );

    let mut children = Vec::new();
    for index in 0..3 {
        let mut command = ql_command(&workspace_root);
        command.current_dir(fixture.temp.path());
        command
            .args(["build"])
            .arg(&fixture.project_root)
            .args(["--package", "app", "--json"]);
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
        let child = command
            .spawn()
            .unwrap_or_else(|error| panic!("spawn concurrent ql build #{index}: {error}"));
        children.push((index, child));
    }

    for (index, child) in children {
        let output = child
            .wait_with_output()
            .unwrap_or_else(|error| panic!("wait for concurrent ql build #{index}: {error}"));
        let (stdout, stderr) = expect_success(
            "project-build-workspace-concurrent-dependency-closure-json",
            &format!("concurrent workspace dependency closure build #{index}"),
            &output,
        )
        .unwrap_or_else(|message| panic!("{message}"));
        expect_empty_stderr(
            "project-build-workspace-concurrent-dependency-closure-json",
            &format!("concurrent workspace dependency closure build #{index}"),
            &stderr,
        )
        .unwrap_or_else(|message| panic!("{message}"));

        let json = parse_json_output(
            "project-build-workspace-concurrent-dependency-closure-json",
            &stdout,
        );
        assert_eq!(
            json["status"], "ok",
            "concurrent build #{index} failed: {json}"
        );
        assert_eq!(json["failure"], JsonValue::Null);
        assert_eq!(
            json["built_targets"]
                .as_array()
                .expect("concurrent build should report built targets")
                .len(),
            2
        );
    }

    expect_file_exists(
        "project-build-workspace-concurrent-dependency-closure-json",
        &fixture.core_output,
        "workspace dependency artifact",
        "concurrent workspace dependency closure build",
    )
    .expect("concurrent builds should emit dependency artifact");
    expect_file_exists(
        "project-build-workspace-concurrent-dependency-closure-json",
        &fixture.app_output,
        "workspace selected package artifact",
        "concurrent workspace dependency closure build",
    )
    .expect("concurrent builds should emit selected package artifact");
    expect_file_exists(
        "project-build-workspace-concurrent-dependency-closure-json",
        &fixture.app_interface_output,
        "workspace selected package interface",
        "concurrent workspace dependency closure build",
    )
    .expect("concurrent builds should emit selected package interface");
    assert_no_build_lock_directories(
        "project-build-workspace-concurrent-dependency-closure-json",
        &fixture.project_root,
    );
    assert_no_atomic_write_temp_files(
        "project-build-workspace-concurrent-dependency-closure-json",
        &fixture.project_root,
    );
}
