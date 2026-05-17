mod support;

use std::path::PathBuf;

use serde_json::Value as JsonValue;
use support::{
    TempDir, expect_empty_stderr, expect_empty_stdout, expect_exit_code, expect_snapshot_matches,
    expect_stderr_contains, expect_stdout_contains_all, expect_success, ql_command,
    read_normalized_file, run_command_capture, workspace_root,
};

fn normalize_output_text(text: &str) -> String {
    text.replace("\r\n", "\n")
}

fn parse_json_output(case_name: &str, stdout: &str) -> JsonValue {
    serde_json::from_str(&normalize_output_text(stdout))
        .unwrap_or_else(|error| panic!("[{case_name}] parse json stdout: {error}\n{stdout}"))
}

fn expect_workspace_lock_json_result(
    json: &JsonValue,
    request_path: &std::path::Path,
    workspace_manifest: &std::path::Path,
    lockfile_path: &std::path::Path,
    check_only: bool,
    status: &str,
) {
    assert_eq!(json["schema"], "ql.project.lock.result.v1");
    assert_eq!(
        json["path"],
        request_path.display().to_string().replace('\\', "/")
    );
    assert_eq!(
        json["project_manifest_path"],
        workspace_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(
        json["lockfile_path"],
        lockfile_path.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["check_only"], check_only);
    assert_eq!(json["status"], status);
    assert_eq!(json["failure"], JsonValue::Null);
    assert_eq!(json["lockfile"]["schema"], "ql.project.lock.v1");
    assert_eq!(json["lockfile"]["root"]["kind"], "workspace");
    assert_eq!(json["lockfile"]["root"]["manifest_path"], "qlang.toml");
}

struct WorkspaceLockFixture {
    project_root: PathBuf,
    workspace_manifest: PathBuf,
    app_member_dir: PathBuf,
    app_source_path: PathBuf,
    workspace_lockfile_path: PathBuf,
    package_lockfile_path: PathBuf,
}

fn write_workspace_lock_fixture(temp: &TempDir) -> WorkspaceLockFixture {
    let project_root = temp.path().join("workspace");
    let app_member_dir = project_root.join("packages/app");
    let app_source_path = app_member_dir.join("src/tools/app.ql");
    let workspace_lockfile_path = project_root.join("qlang.lock");
    let package_lockfile_path = app_member_dir.join("qlang.lock");

    std::fs::create_dir_all(app_member_dir.join("src/tools"))
        .expect("create app source tree for workspace lock fixture");
    std::fs::create_dir_all(project_root.join("packages/core/src/runtime"))
        .expect("create core source tree for workspace lock fixture");
    std::fs::create_dir_all(project_root.join("vendor/runtime/src"))
        .expect("create runtime source tree for workspace lock fixture");

    let workspace_manifest = temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/core"]

[profile]
default = "release"
"#,
    );
    temp.write(
        "workspace/packages/core/qlang.toml",
        r#"
[package]
name = "core"

[lib]
path = "src/runtime/core.ql"
"#,
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"

[dependencies]
core = { path = "../core" }
runtime = { path = "../../vendor/runtime" }

[[bin]]
path = "src/tools/app.ql"
"#,
    );
    temp.write(
        "workspace/vendor/runtime/qlang.toml",
        r#"
[package]
name = "runtime"

[profile]
default = "debug"
"#,
    );
    temp.write(
        "workspace/packages/core/src/runtime/core.ql",
        "pub fn core() -> Int { return 1 }\n",
    );
    temp.write(
        "workspace/packages/app/src/tools/app.ql",
        "fn main() -> Int { return 0 }\n",
    );
    temp.write(
        "workspace/vendor/runtime/src/lib.ql",
        "pub fn runtime() -> Int { return 2 }\n",
    );

    WorkspaceLockFixture {
        project_root,
        workspace_manifest,
        app_member_dir,
        app_source_path,
        workspace_lockfile_path,
        package_lockfile_path,
    }
}

struct PackageLockFixture {
    manifest_path: PathBuf,
    source_path: PathBuf,
    lockfile_path: PathBuf,
}

fn write_package_lock_fixture(temp: &TempDir) -> PackageLockFixture {
    let project_root = temp.path().join("app");
    let source_path = project_root.join("src/main.ql");
    let lockfile_path = project_root.join("qlang.lock");

    std::fs::create_dir_all(project_root.join("src"))
        .expect("create app source tree for package lock fixture");
    std::fs::create_dir_all(temp.path().join("dep/src"))
        .expect("create dependency source tree for package lock fixture");

    let manifest_path = temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"

[dependencies]
dep = { path = "../dep" }

[[bin]]
path = "src/main.ql"
"#,
    );
    temp.write(
        "dep/qlang.toml",
        r#"
[package]
name = "dep"

[lib]
path = "src/lib.ql"
"#,
    );
    temp.write("app/src/main.ql", "fn main() -> Int { return 0 }\n");
    temp.write("dep/src/lib.ql", "pub fn dep() -> Int { return 1 }\n");

    PackageLockFixture {
        manifest_path,
        source_path,
        lockfile_path,
    }
}

#[test]
fn project_lock_writes_workspace_lockfile_with_external_dependencies() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-lock-workspace");
    let fixture = write_workspace_lock_fixture(&temp);
    let lockfile_path = fixture.workspace_lockfile_path;
    let lockfile_display = lockfile_path.to_string_lossy().replace('\\', "/");

    let mut command = ql_command(&workspace_root);
    command.args(["project", "lock"]).arg(&fixture.project_root);
    let output = run_command_capture(&mut command, "`ql project lock` workspace");
    let (stdout, stderr) = expect_success(
        "project-lock-workspace",
        "workspace lockfile generation",
        &output,
    )
    .expect("workspace lockfile generation should succeed");
    expect_empty_stderr(
        "project-lock-workspace",
        "workspace lockfile generation",
        &stderr,
    )
    .expect("workspace lockfile generation should not print stderr");
    expect_stdout_contains_all(
        "project-lock-workspace",
        &stdout.replace('\\', "/"),
        &[&format!("wrote lockfile: {lockfile_display}")],
    )
    .expect("workspace lockfile generation should report the written lockfile path");

    let expected = r#"{
  "packages": [
    {
      "default_profile": "release",
      "dependencies": [],
      "manifest_path": "packages/core/qlang.toml",
      "package_name": "core",
      "selected": true,
      "targets": [
        {
          "kind": "lib",
          "path": "packages/core/src/runtime/core.ql"
        }
      ]
    },
    {
      "default_profile": "debug",
      "dependencies": [],
      "manifest_path": "vendor/runtime/qlang.toml",
      "package_name": "runtime",
      "selected": false,
      "targets": [
        {
          "kind": "lib",
          "path": "vendor/runtime/src/lib.ql"
        }
      ]
    },
    {
      "default_profile": "release",
      "dependencies": [
        "packages/core/qlang.toml",
        "vendor/runtime/qlang.toml"
      ],
      "manifest_path": "packages/app/qlang.toml",
      "package_name": "app",
      "selected": true,
      "targets": [
        {
          "kind": "bin",
          "path": "packages/app/src/tools/app.ql"
        }
      ]
    }
  ],
  "root": {
    "kind": "workspace",
    "manifest_path": "qlang.toml"
  },
  "schema": "ql.project.lock.v1",
  "workspace_members": [
    "packages/core/qlang.toml",
    "packages/app/qlang.toml"
  ]
}
"#;
    let actual = read_normalized_file(&lockfile_path, "workspace lockfile");
    expect_snapshot_matches(
        "project-lock-workspace",
        "workspace lockfile contents",
        expected,
        &actual,
    )
    .expect("workspace lockfile should match the stable schema contract");
}

#[test]
fn project_lock_source_file_uses_workspace_root_context() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-lock-workspace-source");
    let fixture = write_workspace_lock_fixture(&temp);
    let source_path = fixture.app_source_path;
    let workspace_lockfile_path = fixture.workspace_lockfile_path;
    let package_lockfile_path = fixture.package_lockfile_path;
    let workspace_lockfile_display = workspace_lockfile_path.to_string_lossy().replace('\\', "/");

    let mut command = ql_command(&workspace_root);
    command.args(["project", "lock"]).arg(&source_path);
    let output = run_command_capture(
        &mut command,
        "`ql project lock` workspace member source path",
    );
    let (stdout, stderr) = expect_success(
        "project-lock-workspace-source",
        "workspace member source lockfile generation",
        &output,
    )
    .expect("workspace member source lockfile generation should succeed");
    expect_empty_stderr(
        "project-lock-workspace-source",
        "workspace member source lockfile generation",
        &stderr,
    )
    .expect("workspace member source lockfile generation should not print stderr");
    expect_stdout_contains_all(
        "project-lock-workspace-source",
        &stdout.replace('\\', "/"),
        &[&format!("wrote lockfile: {workspace_lockfile_display}")],
    )
    .expect(
        "workspace member source lockfile generation should report the workspace lockfile path",
    );

    assert!(
        !package_lockfile_path.exists(),
        "workspace member source lockfile generation should not create a package-local lockfile at `{}`",
        package_lockfile_path.display()
    );

    let actual = read_normalized_file(&workspace_lockfile_path, "workspace member source lockfile");
    expect_stdout_contains_all(
        "project-lock-workspace-source",
        &actual,
        &[
            "\"kind\": \"workspace\"",
            "\"manifest_path\": \"qlang.toml\"",
            "\"packages/app/qlang.toml\"",
            "\"packages/core/qlang.toml\"",
        ],
    )
    .expect("workspace member source lockfile should preserve workspace root metadata");
}

#[test]
fn project_lock_member_directory_uses_workspace_root_context() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-lock-workspace-member-dir");
    let fixture = write_workspace_lock_fixture(&temp);
    let member_dir = fixture.app_member_dir;
    let workspace_lockfile_path = fixture.workspace_lockfile_path;
    let package_lockfile_path = fixture.package_lockfile_path;
    let workspace_lockfile_display = workspace_lockfile_path.to_string_lossy().replace('\\', "/");

    let mut command = ql_command(&workspace_root);
    command.args(["project", "lock"]).arg(&member_dir);
    let output = run_command_capture(&mut command, "`ql project lock` workspace member directory");
    let (stdout, stderr) = expect_success(
        "project-lock-workspace-member-dir",
        "workspace member directory lockfile generation",
        &output,
    )
    .expect("workspace member directory lockfile generation should succeed");
    expect_empty_stderr(
        "project-lock-workspace-member-dir",
        "workspace member directory lockfile generation",
        &stderr,
    )
    .expect("workspace member directory lockfile generation should not print stderr");
    expect_stdout_contains_all(
        "project-lock-workspace-member-dir",
        &stdout.replace('\\', "/"),
        &[&format!("wrote lockfile: {workspace_lockfile_display}")],
    )
    .expect(
        "workspace member directory lockfile generation should report the workspace lockfile path",
    );

    assert!(
        !package_lockfile_path.exists(),
        "workspace member directory lockfile generation should not create a package-local lockfile at `{}`",
        package_lockfile_path.display()
    );

    let actual = read_normalized_file(
        &workspace_lockfile_path,
        "workspace member directory lockfile",
    );
    expect_stdout_contains_all(
        "project-lock-workspace-member-dir",
        &actual,
        &[
            "\"kind\": \"workspace\"",
            "\"manifest_path\": \"qlang.toml\"",
            "\"packages/app/qlang.toml\"",
            "\"packages/core/qlang.toml\"",
        ],
    )
    .expect("workspace member directory lockfile should preserve workspace root metadata");
}

#[test]
fn project_lock_json_writes_workspace_lockfile() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-lock-workspace-json");
    let fixture = write_workspace_lock_fixture(&temp);
    let project_root = fixture.project_root;
    let workspace_manifest = fixture.workspace_manifest;
    let lockfile_path = fixture.workspace_lockfile_path;

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "lock"])
        .arg(&project_root)
        .arg("--json");
    let output = run_command_capture(&mut command, "`ql project lock --json` workspace");
    let (stdout, stderr) = expect_success(
        "project-lock-workspace-json",
        "workspace lockfile json generation",
        &output,
    )
    .expect("workspace lockfile json generation should succeed");
    expect_empty_stderr(
        "project-lock-workspace-json",
        "workspace lockfile json generation",
        &stderr,
    )
    .expect("workspace lockfile json generation should keep stderr empty");

    let json = parse_json_output("project-lock-workspace-json", &stdout);
    expect_workspace_lock_json_result(
        &json,
        &project_root,
        &workspace_manifest,
        &lockfile_path,
        false,
        "wrote",
    );

    let actual = read_normalized_file(&lockfile_path, "workspace json lockfile");
    let actual_json: JsonValue =
        serde_json::from_str(&actual).expect("written workspace lockfile should remain valid json");
    assert_eq!(json["lockfile"], actual_json);
}

#[test]
fn project_lock_json_accepts_workspace_member_source_path() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-lock-json-workspace-source");
    let fixture = write_workspace_lock_fixture(&temp);
    let workspace_manifest = fixture.workspace_manifest;
    let source_path = fixture.app_source_path;
    let lockfile_path = fixture.workspace_lockfile_path;
    let package_lockfile_path = fixture.package_lockfile_path;

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "lock"])
        .arg(&source_path)
        .arg("--json");
    let output = run_command_capture(
        &mut command,
        "`ql project lock --json` workspace member source path",
    );
    let (stdout, stderr) = expect_success(
        "project-lock-json-workspace-source",
        "workspace member source json lockfile generation",
        &output,
    )
    .expect("workspace member source json lockfile generation should succeed");
    expect_empty_stderr(
        "project-lock-json-workspace-source",
        "workspace member source json lockfile generation",
        &stderr,
    )
    .expect("workspace member source json lockfile generation should keep stderr empty");

    let json = parse_json_output("project-lock-json-workspace-source", &stdout);
    expect_workspace_lock_json_result(
        &json,
        &source_path,
        &workspace_manifest,
        &lockfile_path,
        false,
        "wrote",
    );
    assert!(
        !package_lockfile_path.exists(),
        "workspace member source json lockfile generation should not create a package-local lockfile at `{}`",
        package_lockfile_path.display()
    );

    let actual = read_normalized_file(&lockfile_path, "workspace member source json lockfile");
    let actual_json: JsonValue =
        serde_json::from_str(&actual).expect("written workspace lockfile should remain valid json");
    assert_eq!(json["lockfile"], actual_json);
}

#[test]
fn project_lock_json_accepts_workspace_member_directory() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-lock-json-workspace-member-dir");
    let fixture = write_workspace_lock_fixture(&temp);
    let workspace_manifest = fixture.workspace_manifest;
    let member_dir = fixture.app_member_dir;
    let lockfile_path = fixture.workspace_lockfile_path;
    let package_lockfile_path = fixture.package_lockfile_path;

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "lock"])
        .arg(&member_dir)
        .arg("--json");
    let output = run_command_capture(
        &mut command,
        "`ql project lock --json` workspace member directory",
    );
    let (stdout, stderr) = expect_success(
        "project-lock-json-workspace-member-dir",
        "workspace member directory json lockfile generation",
        &output,
    )
    .expect("workspace member directory json lockfile generation should succeed");
    expect_empty_stderr(
        "project-lock-json-workspace-member-dir",
        "workspace member directory json lockfile generation",
        &stderr,
    )
    .expect("workspace member directory json lockfile generation should keep stderr empty");

    let json = parse_json_output("project-lock-json-workspace-member-dir", &stdout);
    expect_workspace_lock_json_result(
        &json,
        &member_dir,
        &workspace_manifest,
        &lockfile_path,
        false,
        "wrote",
    );
    assert!(
        !package_lockfile_path.exists(),
        "workspace member directory json lockfile generation should not create a package-local lockfile at `{}`",
        package_lockfile_path.display()
    );

    let actual = read_normalized_file(&lockfile_path, "workspace member directory json lockfile");
    let actual_json: JsonValue =
        serde_json::from_str(&actual).expect("written workspace lockfile should remain valid json");
    assert_eq!(json["lockfile"], actual_json);
}

#[test]
fn project_lock_check_json_accepts_workspace_member_source_path() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-lock-check-json-workspace-source");
    let fixture = write_workspace_lock_fixture(&temp);
    let project_root = fixture.project_root;
    let workspace_manifest = fixture.workspace_manifest;
    let source_path = fixture.app_source_path;
    let lockfile_path = fixture.workspace_lockfile_path;
    let package_lockfile_path = fixture.package_lockfile_path;

    let mut write_command = ql_command(&workspace_root);
    write_command.args(["project", "lock"]).arg(&project_root);
    let write_output = run_command_capture(
        &mut write_command,
        "`ql project lock` before member source check",
    );
    let (_, write_stderr) = expect_success(
        "project-lock-check-json-workspace-source",
        "initial workspace lockfile generation",
        &write_output,
    )
    .expect("initial workspace lockfile generation should succeed");
    expect_empty_stderr(
        "project-lock-check-json-workspace-source",
        "initial workspace lockfile generation",
        &write_stderr,
    )
    .expect("initial workspace lockfile generation should not print stderr");
    let initial_lockfile = read_normalized_file(&lockfile_path, "initial workspace lockfile");

    let mut check_command = ql_command(&workspace_root);
    check_command
        .args(["project", "lock", "--check"])
        .arg(&source_path)
        .arg("--json");
    let output = run_command_capture(
        &mut check_command,
        "`ql project lock --check --json` workspace member source path",
    );
    let (stdout, stderr) = expect_success(
        "project-lock-check-json-workspace-source",
        "workspace member source json lockfile check",
        &output,
    )
    .expect("workspace member source json lockfile check should succeed");
    expect_empty_stderr(
        "project-lock-check-json-workspace-source",
        "workspace member source json lockfile check",
        &stderr,
    )
    .expect("workspace member source json lockfile check should keep stderr empty");

    let json = parse_json_output("project-lock-check-json-workspace-source", &stdout);
    expect_workspace_lock_json_result(
        &json,
        &source_path,
        &workspace_manifest,
        &lockfile_path,
        true,
        "up-to-date",
    );

    let checked_lockfile = read_normalized_file(&lockfile_path, "checked workspace lockfile");
    expect_snapshot_matches(
        "project-lock-check-json-workspace-source",
        "workspace member source json check lockfile contents",
        &initial_lockfile,
        &checked_lockfile,
    )
    .expect("workspace member source json lockfile check should not rewrite the lockfile");
    assert!(
        !package_lockfile_path.exists(),
        "workspace member source json lockfile check should not create a package-local lockfile at `{}`",
        package_lockfile_path.display()
    );
}

#[test]
fn project_lock_check_json_accepts_workspace_member_directory() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-lock-check-json-workspace-member-dir");
    let fixture = write_workspace_lock_fixture(&temp);
    let project_root = fixture.project_root;
    let workspace_manifest = fixture.workspace_manifest;
    let member_dir = fixture.app_member_dir;
    let lockfile_path = fixture.workspace_lockfile_path;
    let package_lockfile_path = fixture.package_lockfile_path;

    let mut write_command = ql_command(&workspace_root);
    write_command.args(["project", "lock"]).arg(&project_root);
    let write_output = run_command_capture(
        &mut write_command,
        "`ql project lock` before member directory check",
    );
    let (_, write_stderr) = expect_success(
        "project-lock-check-json-workspace-member-dir",
        "initial workspace lockfile generation",
        &write_output,
    )
    .expect("initial workspace lockfile generation should succeed");
    expect_empty_stderr(
        "project-lock-check-json-workspace-member-dir",
        "initial workspace lockfile generation",
        &write_stderr,
    )
    .expect("initial workspace lockfile generation should not print stderr");
    let initial_lockfile = read_normalized_file(&lockfile_path, "initial workspace lockfile");

    let mut check_command = ql_command(&workspace_root);
    check_command
        .args(["project", "lock", "--check"])
        .arg(&member_dir)
        .arg("--json");
    let output = run_command_capture(
        &mut check_command,
        "`ql project lock --check --json` workspace member directory",
    );
    let (stdout, stderr) = expect_success(
        "project-lock-check-json-workspace-member-dir",
        "workspace member directory json lockfile check",
        &output,
    )
    .expect("workspace member directory json lockfile check should succeed");
    expect_empty_stderr(
        "project-lock-check-json-workspace-member-dir",
        "workspace member directory json lockfile check",
        &stderr,
    )
    .expect("workspace member directory json lockfile check should keep stderr empty");

    let json = parse_json_output("project-lock-check-json-workspace-member-dir", &stdout);
    expect_workspace_lock_json_result(
        &json,
        &member_dir,
        &workspace_manifest,
        &lockfile_path,
        true,
        "up-to-date",
    );

    let checked_lockfile = read_normalized_file(&lockfile_path, "checked workspace lockfile");
    expect_snapshot_matches(
        "project-lock-check-json-workspace-member-dir",
        "workspace member directory json check lockfile contents",
        &initial_lockfile,
        &checked_lockfile,
    )
    .expect("workspace member directory json lockfile check should not rewrite the lockfile");
    assert!(
        !package_lockfile_path.exists(),
        "workspace member directory json lockfile check should not create a package-local lockfile at `{}`",
        package_lockfile_path.display()
    );
}

#[test]
fn project_lock_json_writes_and_checks_package_lockfile_from_source_path() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-lock-package-source-json");
    let fixture = write_package_lock_fixture(&temp);

    let mut write_command = ql_command(&workspace_root);
    write_command
        .args(["project", "lock"])
        .arg(&fixture.source_path)
        .arg("--json");
    let output = run_command_capture(
        &mut write_command,
        "`ql project lock --json` package source path",
    );
    let (stdout, stderr) = expect_success(
        "project-lock-package-source-json",
        "package source json lockfile generation",
        &output,
    )
    .expect("package source json lockfile generation should succeed");
    expect_empty_stderr(
        "project-lock-package-source-json",
        "package source json lockfile generation",
        &stderr,
    )
    .expect("package source json lockfile generation should keep stderr empty");

    let json = parse_json_output("project-lock-package-source-json", &stdout);
    assert_eq!(json["schema"], "ql.project.lock.result.v1");
    assert_eq!(
        json["path"],
        fixture.source_path.display().to_string().replace('\\', "/")
    );
    assert_eq!(
        json["project_manifest_path"],
        fixture
            .manifest_path
            .display()
            .to_string()
            .replace('\\', "/")
    );
    assert_eq!(
        json["lockfile_path"],
        fixture
            .lockfile_path
            .display()
            .to_string()
            .replace('\\', "/")
    );
    assert_eq!(json["check_only"], false);
    assert_eq!(json["status"], "wrote");
    assert_eq!(json["failure"], JsonValue::Null);
    assert_eq!(json["lockfile"]["schema"], "ql.project.lock.v1");
    assert_eq!(json["lockfile"]["root"]["kind"], "package");
    assert_eq!(json["lockfile"]["root"]["manifest_path"], "qlang.toml");

    let package_names: Vec<&str> = json["lockfile"]["packages"]
        .as_array()
        .expect("package lockfile should expose package entries")
        .iter()
        .map(|package| {
            package["package_name"]
                .as_str()
                .expect("package entry should expose package_name")
        })
        .collect();
    assert!(
        package_names.contains(&"app") && package_names.contains(&"dep"),
        "package lockfile should include the package and local dependency: {json}"
    );

    let initial_lockfile =
        read_normalized_file(&fixture.lockfile_path, "package source json lockfile");
    let written_lockfile: JsonValue = serde_json::from_str(&initial_lockfile)
        .expect("written package lockfile should remain valid json");
    assert_eq!(json["lockfile"], written_lockfile);

    let mut check_command = ql_command(&workspace_root);
    check_command
        .args(["project", "lock", "--check"])
        .arg(&fixture.source_path)
        .arg("--json");
    let output = run_command_capture(
        &mut check_command,
        "`ql project lock --check --json` package source path",
    );
    let (stdout, stderr) = expect_success(
        "project-lock-package-source-json",
        "package source json lockfile check",
        &output,
    )
    .expect("package source json lockfile check should succeed");
    expect_empty_stderr(
        "project-lock-package-source-json",
        "package source json lockfile check",
        &stderr,
    )
    .expect("package source json lockfile check should keep stderr empty");

    let json = parse_json_output("project-lock-package-source-json-check", &stdout);
    assert_eq!(json["schema"], "ql.project.lock.result.v1");
    assert_eq!(json["check_only"], true);
    assert_eq!(json["status"], "up-to-date");
    assert_eq!(json["failure"], JsonValue::Null);
    assert_eq!(json["lockfile"], written_lockfile);

    let checked_lockfile = read_normalized_file(
        &fixture.lockfile_path,
        "checked package source json lockfile",
    );
    expect_snapshot_matches(
        "project-lock-package-source-json",
        "checked package source json lockfile contents",
        &initial_lockfile,
        &checked_lockfile,
    )
    .expect("package source json lockfile check should not rewrite the lockfile");
}

#[test]
fn project_lock_json_reports_invalid_manifest_load_failure() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-lock-invalid-manifest-json");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(&project_root)
        .expect("create project directory for invalid lock manifest test");
    let manifest_path = temp.write(
        "app/qlang.toml",
        r#"
[package
name = "app"
"#,
    );

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "lock"])
        .arg(&project_root)
        .arg("--json");
    let output = run_command_capture(&mut command, "`ql project lock --json` invalid manifest");
    let (stdout, stderr) = expect_exit_code(
        "project-lock-invalid-manifest-json",
        "invalid manifest json lockfile generation",
        &output,
        1,
    )
    .expect("invalid manifest json lockfile generation should fail");
    expect_empty_stderr(
        "project-lock-invalid-manifest-json",
        "invalid manifest json lockfile generation",
        &stderr,
    )
    .expect("invalid manifest json lockfile generation should keep stderr empty");

    let json = parse_json_output("project-lock-invalid-manifest-json", &stdout);
    assert_eq!(json["schema"], "ql.project.lock.result.v1");
    assert_eq!(json["status"], "failed");
    assert_eq!(json["check_only"], false);
    assert_eq!(json["lockfile"], JsonValue::Null);
    assert_eq!(json["lockfile_path"], JsonValue::Null);
    assert_eq!(
        json["project_manifest_path"],
        manifest_path.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["failure"]["kind"], "preflight");
    let failure = &json["failure"]["preflight_failure"];
    assert_eq!(failure["stage"], "manifest-load");
    assert_eq!(
        failure["manifest_path"],
        manifest_path.display().to_string().replace('\\', "/")
    );
    assert!(
        failure["message"]
            .as_str()
            .expect("invalid manifest json failure should expose a message")
            .contains("invalid manifest"),
        "invalid manifest json failure should describe the load failure: {json}"
    );
}

#[test]
fn project_lock_json_reports_lockfile_render_failure() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-lock-render-failure-json");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(&project_root)
        .expect("create project directory for render failure lock test");
    let manifest_path = temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    let lockfile_path = project_root.join("qlang.lock");

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "lock"])
        .arg(&project_root)
        .arg("--json");
    let output = run_command_capture(&mut command, "`ql project lock --json` render failure");
    let (stdout, stderr) = expect_exit_code(
        "project-lock-render-failure-json",
        "render failure json lockfile generation",
        &output,
        1,
    )
    .expect("render failure json lockfile generation should fail");
    expect_empty_stderr(
        "project-lock-render-failure-json",
        "render failure json lockfile generation",
        &stderr,
    )
    .expect("render failure json lockfile generation should keep stderr empty");
    assert!(
        !lockfile_path.exists(),
        "render failure json lockfile generation should not create a lockfile"
    );

    let json = parse_json_output("project-lock-render-failure-json", &stdout);
    assert_eq!(json["schema"], "ql.project.lock.result.v1");
    assert_eq!(json["status"], "failed");
    assert_eq!(json["check_only"], false);
    assert_eq!(json["lockfile"], JsonValue::Null);
    assert_eq!(
        json["project_manifest_path"],
        manifest_path.display().to_string().replace('\\', "/")
    );
    assert_eq!(
        json["lockfile_path"],
        lockfile_path.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["failure"]["kind"], "preflight");
    let failure = &json["failure"]["preflight_failure"];
    assert_eq!(failure["stage"], "lockfile-render");
    assert_eq!(
        failure["manifest_path"],
        manifest_path.display().to_string().replace('\\', "/")
    );
    assert!(
        failure["message"]
            .as_str()
            .expect("render failure json should expose a message")
            .contains("package source directory"),
        "render failure json should describe the missing package source root: {json}"
    );
}

#[test]
fn project_lock_check_fails_when_lockfile_is_stale() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-lock-stale");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create package source tree for stale lock test");

    let manifest_path = temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"

[profile]
default = "release"
"#,
    );
    temp.write("app/src/main.ql", "fn main() -> Int { return 0 }\n");

    let lockfile_path = project_root.join("qlang.lock");
    let mut write_command = ql_command(&workspace_root);
    write_command.args(["project", "lock"]).arg(&project_root);
    let write_output = run_command_capture(&mut write_command, "`ql project lock` package");
    let (_, write_stderr) = expect_success(
        "project-lock-stale",
        "initial lockfile generation",
        &write_output,
    )
    .expect("initial lockfile generation should succeed");
    expect_empty_stderr(
        "project-lock-stale",
        "initial lockfile generation",
        &write_stderr,
    )
    .expect("initial lockfile generation should not print stderr");
    let initial_lockfile = read_normalized_file(&lockfile_path, "initial lockfile");

    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"

[profile]
default = "debug"
"#,
    );

    let mut check_command = ql_command(&workspace_root);
    check_command
        .args(["project", "lock", "--check"])
        .arg(&project_root);
    let output = run_command_capture(&mut check_command, "`ql project lock --check` stale");
    let (stdout, stderr) =
        expect_exit_code("project-lock-stale", "stale lockfile check", &output, 1)
            .expect("stale lockfile check should fail with exit code 1");
    expect_empty_stdout("project-lock-stale", "stale lockfile check", &stdout)
        .expect("stale lockfile check should not print stdout");

    let normalized_stderr = stderr.replace('\\', "/");
    let lockfile_display = lockfile_path.to_string_lossy().replace('\\', "/");
    let manifest_display = manifest_path.to_string_lossy().replace('\\', "/");
    expect_stderr_contains(
        "project-lock-stale",
        "stale lockfile check",
        &normalized_stderr,
        &format!("error: `ql project lock --check` lockfile `{lockfile_display}` is stale"),
    )
    .expect("stale lockfile check should report a stale lockfile");
    expect_stderr_contains(
        "project-lock-stale",
        "stale lockfile check",
        &normalized_stderr,
        &format!("note: failing package manifest: {manifest_display}"),
    )
    .expect("stale lockfile check should point to the package manifest");
    expect_stderr_contains(
        "project-lock-stale",
        "stale lockfile check",
        &normalized_stderr,
        &format!("hint: rerun `ql project lock {manifest_display}` to regenerate `qlang.lock`"),
    )
    .expect("stale lockfile check should preserve the direct regeneration hint");

    let unchanged_lockfile = read_normalized_file(&lockfile_path, "stale lockfile");
    expect_snapshot_matches(
        "project-lock-stale",
        "stale lockfile contents",
        &initial_lockfile,
        &unchanged_lockfile,
    )
    .expect("stale lockfile check should not rewrite the existing lockfile");
}

#[test]
fn project_lock_check_json_reports_stale_lockfile() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-lock-stale-json");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create package source tree for stale json lock test");

    let manifest_path = temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"

[profile]
default = "release"
"#,
    );
    temp.write("app/src/main.ql", "fn main() -> Int { return 0 }\n");

    let lockfile_path = project_root.join("qlang.lock");
    let mut write_command = ql_command(&workspace_root);
    write_command.args(["project", "lock"]).arg(&project_root);
    let write_output = run_command_capture(&mut write_command, "`ql project lock` package");
    let (_, write_stderr) = expect_success(
        "project-lock-stale-json",
        "initial json lockfile generation",
        &write_output,
    )
    .expect("initial json lockfile generation should succeed");
    expect_empty_stderr(
        "project-lock-stale-json",
        "initial json lockfile generation",
        &write_stderr,
    )
    .expect("initial json lockfile generation should not print stderr");
    let initial_lockfile = read_normalized_file(&lockfile_path, "initial json lockfile");

    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"

[profile]
default = "debug"
"#,
    );

    let mut check_command = ql_command(&workspace_root);
    check_command
        .args(["project", "lock", "--check"])
        .arg(&project_root)
        .arg("--json");
    let output = run_command_capture(&mut check_command, "`ql project lock --check --json` stale");
    let (stdout, stderr) = expect_exit_code(
        "project-lock-stale-json",
        "stale json lockfile check",
        &output,
        1,
    )
    .expect("stale json lockfile check should fail with exit code 1");
    expect_empty_stderr(
        "project-lock-stale-json",
        "stale json lockfile check",
        &stderr,
    )
    .expect("stale json lockfile check should keep stderr empty");

    let json = parse_json_output("project-lock-stale-json", &stdout);
    assert_eq!(json["schema"], "ql.project.lock.result.v1");
    assert_eq!(
        json["path"],
        project_root.display().to_string().replace('\\', "/")
    );
    assert_eq!(
        json["project_manifest_path"],
        manifest_path.display().to_string().replace('\\', "/")
    );
    assert_eq!(
        json["lockfile_path"],
        lockfile_path.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["check_only"], true);
    assert_eq!(json["status"], "failed");
    assert_eq!(json["failure"]["kind"], "stale");
    assert_eq!(
        json["failure"]["message"],
        format!(
            "lockfile `{}` is stale",
            lockfile_path.display().to_string().replace('\\', "/")
        )
    );
    assert_eq!(
        json["failure"]["rerun_command"],
        format!(
            "ql project lock {}",
            manifest_path.display().to_string().replace('\\', "/")
        )
    );
    assert_eq!(json["lockfile"]["schema"], "ql.project.lock.v1");
    assert_eq!(json["lockfile"]["packages"][0]["default_profile"], "debug");

    let unchanged_lockfile = read_normalized_file(&lockfile_path, "stale json lockfile");
    expect_snapshot_matches(
        "project-lock-stale-json",
        "stale json lockfile contents",
        &initial_lockfile,
        &unchanged_lockfile,
    )
    .expect("stale json lockfile check should not rewrite the existing lockfile");
}
