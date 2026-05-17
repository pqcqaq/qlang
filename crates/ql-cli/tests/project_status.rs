mod support;

use serde_json::Value as JsonValue;
use support::{
    TempDir, expect_empty_stderr, expect_empty_stdout, expect_exit_code, expect_stderr_contains,
    expect_stdout_contains_all, expect_success, normalize, ql_command, run_command_capture,
    workspace_root,
};

fn parse_json_output(case_name: &str, stdout: &str) -> JsonValue {
    serde_json::from_str(&normalize(stdout))
        .unwrap_or_else(|error| panic!("[{case_name}] parse json stdout: {error}\n{stdout}"))
}

struct WorkspaceStatusSelectorFixture {
    _temp: TempDir,
    project_root: std::path::PathBuf,
}

struct StandaloneStatusFixture {
    project_root: std::path::PathBuf,
    manifest_path: std::path::PathBuf,
    source_path: std::path::PathBuf,
}

fn write_status_workspace(temp: &TempDir) -> std::path::PathBuf {
    let project_root = temp.path().join("workspace");
    temp.write(
        "workspace/qlang.toml",
        "[workspace]\nmembers = [\"packages/app\", \"packages/core\"]\n",
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        "[dependencies]\ncore = \"../core\"\n\"vendor.core\" = \"../../vendor/core\"\n\n[package]\nname = \"app\"\n",
    );
    temp.write(
        "workspace/packages/app/src/main.ql",
        "fn main() -> Int {\n    return 0\n}\n",
    );
    temp.write(
        "workspace/packages/core/qlang.toml",
        "[package]\nname = \"core\"\n",
    );
    temp.write(
        "workspace/packages/core/src/lib.ql",
        "pub fn answer() -> Int {\n    return 42\n}\n",
    );
    temp.write(
        "workspace/vendor/core/qlang.toml",
        "[package]\nname = \"vendor.core\"\n",
    );
    project_root
}

fn write_status_selector_workspace(temp: TempDir) -> WorkspaceStatusSelectorFixture {
    let project_root = write_status_workspace(&temp);
    WorkspaceStatusSelectorFixture {
        _temp: temp,
        project_root,
    }
}

fn write_standalone_status_package(temp: &TempDir) -> StandaloneStatusFixture {
    let project_root = temp.path().join("app");
    let manifest_path = temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"

[profile]
default = "release"

[dependencies]
dep = { path = "../dep" }

[lib]
path = "src/lib.ql"
"#,
    );
    let source_path = temp.write(
        "app/src/lib.ql",
        "pub fn app_value() -> Int {\n    return 1\n}\n",
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
    temp.write(
        "dep/src/lib.ql",
        "pub fn dep_value() -> Int {\n    return 2\n}\n",
    );

    StandaloneStatusFixture {
        project_root,
        manifest_path,
        source_path,
    }
}

fn assert_standalone_status_json(
    case_name: &str,
    stdout: &str,
    request_path: &std::path::Path,
    fixture: &StandaloneStatusFixture,
) {
    let actual = parse_json_output(case_name, stdout);
    assert_eq!(actual["schema"], "ql.project.status.v1");
    assert_eq!(
        actual["path"],
        request_path.to_string_lossy().replace('\\', "/")
    );
    assert_eq!(
        actual["project_manifest_path"],
        fixture.manifest_path.to_string_lossy().replace('\\', "/")
    );
    assert_eq!(actual["kind"], "package");
    assert_eq!(actual["status"], "needs-interface-sync");

    let members = actual["members"]
        .as_array()
        .expect("standalone status members should be an array");
    assert_eq!(
        members.len(),
        1,
        "standalone package should report one member"
    );
    let app = &members[0];
    assert_eq!(app["member"], JsonValue::Null);
    assert_eq!(app["package_name"], "app");
    assert_eq!(
        app["manifest_path"],
        fixture.manifest_path.to_string_lossy().replace('\\', "/")
    );
    assert_eq!(app["default_profile"], "release");
    assert_eq!(app["interface"]["status"], "missing");
    assert_eq!(app["targets"][0]["kind"], "lib");
    assert_eq!(app["targets"][0]["path"], "src/lib.ql");
    assert_eq!(app["dependencies"][0]["kind"], "local");
    assert_eq!(app["dependencies"][0]["member"], JsonValue::Null);
    assert_eq!(app["dependencies"][0]["package_name"], "dep");
    assert_eq!(app["dependencies"][0]["dependency_path"], "../dep");
}

#[test]
fn project_status_reports_workspace_members_targets_dependencies_and_interfaces_as_json() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-status-json");
    let project_root = write_status_workspace(&temp);

    let mut command = ql_command(&workspace_root);
    command.args([
        "project",
        "status",
        &project_root.to_string_lossy(),
        "--json",
    ]);
    let output = run_command_capture(&mut command, "`ql project status --json` workspace");
    let (stdout, stderr) =
        expect_success("project-status-json", "project status json", &output).unwrap();
    expect_empty_stderr("project-status-json", "project status json", &stderr).unwrap();

    let actual = parse_json_output("project-status-json", &stdout);
    assert_eq!(actual["schema"], "ql.project.status.v1");
    assert_eq!(actual["kind"], "workspace");
    assert_eq!(actual["status"], "needs-interface-sync");

    let members = actual["members"]
        .as_array()
        .expect("project status members should be an array");
    assert_eq!(
        members.len(),
        2,
        "project status should report both members"
    );
    let app = members
        .iter()
        .find(|member| member["package_name"] == "app")
        .expect("status should include app member");
    assert_eq!(app["member"], "packages/app");
    assert_eq!(app["interface"]["status"], "missing");
    assert_eq!(app["targets"][0]["kind"], "bin");
    assert_eq!(app["targets"][0]["path"], "src/main.ql");
    assert_eq!(app["dependencies"][0]["kind"], "workspace");
    assert_eq!(app["dependencies"][0]["member"], "packages/core");
    assert_eq!(app["dependencies"][0]["dependency_path"], "../core");
    assert_eq!(app["dependencies"][1]["kind"], "local");
    assert_eq!(app["dependencies"][1]["member"], JsonValue::Null);
    assert_eq!(
        app["dependencies"][1]["dependency_path"],
        "../../vendor/core"
    );

    let core = members
        .iter()
        .find(|member| member["package_name"] == "core")
        .expect("status should include core member");
    assert_eq!(
        core["dependencies"].as_array().map(Vec::len),
        Some(0),
        "core should not report dependencies"
    );
}

#[test]
fn project_status_reports_standalone_package_as_json() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-status-package-json");
    let fixture = write_standalone_status_package(&temp);

    let mut command = ql_command(&workspace_root);
    command.args([
        "project",
        "status",
        &fixture.project_root.to_string_lossy(),
        "--json",
    ]);
    let output = run_command_capture(&mut command, "`ql project status --json` package");
    let (stdout, stderr) = expect_success(
        "project-status-package-json",
        "project status standalone package json",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-status-package-json",
        "project status standalone package json",
        &stderr,
    )
    .unwrap();

    assert_standalone_status_json(
        "project-status-package-json",
        &stdout,
        &fixture.project_root,
        &fixture,
    );
}

#[test]
fn project_status_reports_standalone_package_source_path_as_json() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-status-package-source-json");
    let fixture = write_standalone_status_package(&temp);

    let mut command = ql_command(&workspace_root);
    command.args([
        "project",
        "status",
        &fixture.source_path.to_string_lossy(),
        "--json",
    ]);
    let output = run_command_capture(
        &mut command,
        "`ql project status --json` package source path",
    );
    let (stdout, stderr) = expect_success(
        "project-status-package-source-json",
        "project status standalone package source path json",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-status-package-source-json",
        "project status standalone package source path json",
        &stderr,
    )
    .unwrap();

    assert_standalone_status_json(
        "project-status-package-source-json",
        &stdout,
        &fixture.source_path,
        &fixture,
    );
}

#[test]
fn project_status_json_reports_invalid_manifest_load_failure() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-status-invalid-manifest-json");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(&project_root)
        .expect("create project directory for invalid manifest status json test");
    let manifest_path = temp.write(
        "workspace/app/qlang.toml",
        r#"
[package
name = "app"
"#,
    );

    let mut command = ql_command(&workspace_root);
    command.args([
        "project",
        "status",
        &project_root.to_string_lossy(),
        "--json",
    ]);
    let output = run_command_capture(&mut command, "`ql project status --json` invalid manifest");
    let (stdout, stderr) = expect_exit_code(
        "project-status-invalid-manifest-json",
        "project status invalid manifest json",
        &output,
        1,
    )
    .unwrap();
    expect_empty_stderr(
        "project-status-invalid-manifest-json",
        "project status invalid manifest json",
        &stderr,
    )
    .unwrap();

    let json = parse_json_output("project-status-invalid-manifest-json", &stdout);
    assert_eq!(json["schema"], "ql.project.status.v1");
    assert_eq!(json["status"], "failed");
    assert_eq!(json["kind"], JsonValue::Null);
    assert_eq!(
        json["project_manifest_path"],
        manifest_path.to_string_lossy().replace('\\', "/")
    );
    assert_eq!(json["members"], serde_json::json!([]));
    assert_eq!(json["failure"]["kind"], "preflight");
    let failure = &json["failure"]["preflight_failure"];
    assert_eq!(failure["stage"], "manifest-load");
    assert_eq!(
        failure["manifest_path"],
        manifest_path.to_string_lossy().replace('\\', "/")
    );
    assert!(
        failure["message"]
            .as_str()
            .expect("project status json invalid manifest should expose a message")
            .contains("invalid manifest"),
        "project status json invalid manifest should describe the load failure: {json}"
    );
}

#[test]
fn project_status_supports_workspace_package_selector() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-status-package-selector");
    let project_root = write_status_workspace(&temp);

    let mut command = ql_command(&workspace_root);
    command.args([
        "project",
        "status",
        &project_root.to_string_lossy(),
        "--package",
        "core",
    ]);
    let output = run_command_capture(
        &mut command,
        "`ql project status --package` workspace selector",
    );
    let (stdout, stderr) = expect_success(
        "project-status-package-selector",
        "project status package selector",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-status-package-selector",
        "project status package selector",
        &stderr,
    )
    .unwrap();

    let stdout = stdout.replace('\\', "/");
    expect_stdout_contains_all(
        "project-status-package-selector",
        &stdout,
        &[
            "status: needs-interface-sync",
            "  - packages/core (core)",
            "    interface: missing",
            "    targets:",
            "      - lib: src/lib.ql",
            "    dependencies: []",
        ],
    )
    .unwrap();
    assert!(
        !stdout.contains("packages/app"),
        "package selector should not include unselected members, got:\n{stdout}"
    );
}

#[test]
fn project_status_supports_json_workspace_package_selector() {
    let workspace_root = workspace_root();
    let fixture = write_status_selector_workspace(TempDir::new(
        "ql-cli-project-status-package-selector-json",
    ));

    let mut command = ql_command(&workspace_root);
    command.args([
        "project",
        "status",
        &fixture.project_root.to_string_lossy(),
        "--package",
        "core",
        "--json",
    ]);
    let output = run_command_capture(
        &mut command,
        "`ql project status --package --json` workspace selector",
    );
    let (stdout, stderr) = expect_success(
        "project-status-package-selector-json",
        "project status package selector json",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-status-package-selector-json",
        "project status package selector json",
        &stderr,
    )
    .unwrap();

    let actual = parse_json_output("project-status-package-selector-json", &stdout);
    let expected = serde_json::json!({
        "schema": "ql.project.status.v1",
        "path": fixture.project_root.to_string_lossy().replace('\\', "/"),
        "project_manifest_path": fixture.project_root.join("qlang.toml").to_string_lossy().replace('\\', "/"),
        "kind": "workspace",
        "status": "needs-interface-sync",
        "members": [
            {
                "member": "packages/core",
                "package_name": "core",
                "manifest_path": fixture.project_root.join("packages/core/qlang.toml").to_string_lossy().replace('\\', "/"),
                "default_profile": JsonValue::Null,
                "interface": {
                    "path": fixture.project_root.join("packages/core/core.qi").to_string_lossy().replace('\\', "/"),
                    "status": "missing",
                    "detail": JsonValue::Null,
                    "stale_reasons": [],
                },
                "targets": [
                    {
                        "kind": "lib",
                        "path": "src/lib.ql",
                    }
                ],
                "dependencies": [],
            }
        ],
    });
    assert_eq!(
        actual, expected,
        "workspace root package selector status json should match the stable contract"
    );
}

#[test]
fn project_status_source_file_uses_workspace_root_context() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-status-source-path");
    let project_root = write_status_workspace(&temp);
    let request_path = project_root.join("packages/app/src/main.ql");

    let mut command = ql_command(&workspace_root);
    command.args([
        "project",
        "status",
        &request_path.to_string_lossy(),
        "--json",
    ]);
    let output = run_command_capture(
        &mut command,
        "`ql project status --json` workspace member source path",
    );
    let (stdout, stderr) = expect_success(
        "project-status-source-path",
        "project status source path",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-status-source-path",
        "project status source path",
        &stderr,
    )
    .unwrap();

    let actual = parse_json_output("project-status-source-path", &stdout);
    assert_eq!(
        actual["path"],
        request_path.to_string_lossy().replace('\\', "/")
    );
    assert_eq!(
        actual["project_manifest_path"],
        project_root
            .join("qlang.toml")
            .to_string_lossy()
            .replace('\\', "/")
    );
    let members = actual["members"]
        .as_array()
        .expect("project status source path members should be an array");
    assert_eq!(
        members.len(),
        2,
        "workspace member source path should keep the outer workspace context"
    );
}

#[test]
fn project_status_member_directory_uses_workspace_root_context() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-status-member-dir");
    let project_root = write_status_workspace(&temp);
    let request_path = project_root.join("packages/app");

    let mut command = ql_command(&workspace_root);
    command.args([
        "project",
        "status",
        &request_path.to_string_lossy(),
        "--json",
    ]);
    let output = run_command_capture(
        &mut command,
        "`ql project status --json` workspace member directory",
    );
    let (stdout, stderr) = expect_success(
        "project-status-member-dir",
        "project status member directory",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-status-member-dir",
        "project status member directory",
        &stderr,
    )
    .unwrap();

    let actual = parse_json_output("project-status-member-dir", &stdout);
    assert_eq!(
        actual["path"],
        request_path.to_string_lossy().replace('\\', "/")
    );
    assert_eq!(
        actual["project_manifest_path"],
        project_root
            .join("qlang.toml")
            .to_string_lossy()
            .replace('\\', "/")
    );
    let members = actual["members"]
        .as_array()
        .expect("project status member directory members should be an array");
    assert_eq!(
        members.len(),
        2,
        "workspace member directory should keep the outer workspace context"
    );
}

#[test]
fn project_status_member_directory_supports_workspace_package_selector() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-status-member-dir-selector");
    let project_root = write_status_workspace(&temp);
    let request_path = project_root.join("packages/app");

    let mut command = ql_command(&workspace_root);
    command.args([
        "project",
        "status",
        &request_path.to_string_lossy(),
        "--package",
        "core",
        "--json",
    ]);
    let output = run_command_capture(
        &mut command,
        "`ql project status --package --json` workspace member directory",
    );
    let (stdout, stderr) = expect_success(
        "project-status-member-dir-selector",
        "project status member directory package selector",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-status-member-dir-selector",
        "project status member directory package selector",
        &stderr,
    )
    .unwrap();

    let actual = parse_json_output("project-status-member-dir-selector", &stdout);
    assert_eq!(
        actual["path"],
        request_path.to_string_lossy().replace('\\', "/")
    );
    assert_eq!(
        actual["project_manifest_path"],
        project_root
            .join("qlang.toml")
            .to_string_lossy()
            .replace('\\', "/")
    );
    let members = actual["members"]
        .as_array()
        .expect("project status member directory selector members should be an array");
    assert_eq!(
        members.len(),
        1,
        "workspace member directory package selector should keep the enclosing workspace"
    );
    assert_eq!(members[0]["member"], "packages/core");
    assert_eq!(members[0]["package_name"], "core");
}

#[test]
fn project_status_source_file_supports_workspace_package_selector() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-status-source-selector");
    let project_root = write_status_workspace(&temp);
    let request_path = project_root.join("packages/app/src/main.ql");

    let mut command = ql_command(&workspace_root);
    command.args([
        "project",
        "status",
        &request_path.to_string_lossy(),
        "--package",
        "core",
        "--json",
    ]);
    let output = run_command_capture(
        &mut command,
        "`ql project status --package --json` workspace member source path",
    );
    let (stdout, stderr) = expect_success(
        "project-status-source-selector",
        "project status source path package selector",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-status-source-selector",
        "project status source path package selector",
        &stderr,
    )
    .unwrap();

    let actual = parse_json_output("project-status-source-selector", &stdout);
    assert_eq!(
        actual["path"],
        request_path.to_string_lossy().replace('\\', "/")
    );
    assert_eq!(
        actual["project_manifest_path"],
        project_root
            .join("qlang.toml")
            .to_string_lossy()
            .replace('\\', "/")
    );
    let members = actual["members"]
        .as_array()
        .expect("project status source selector members should be an array");
    assert_eq!(
        members.len(),
        1,
        "workspace member source package selector should keep the enclosing workspace"
    );
    assert_eq!(members[0]["member"], "packages/core");
    assert_eq!(members[0]["package_name"], "core");
}

#[test]
fn project_status_workspace_root_package_selector_reports_missing_package() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-status-missing-package");
    let project_root = write_status_workspace(&temp);

    let mut command = ql_command(&workspace_root);
    command.args([
        "project",
        "status",
        &project_root.to_string_lossy(),
        "--package",
        "missing",
    ]);
    let output = run_command_capture(
        &mut command,
        "`ql project status --package` missing workspace package",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-status-missing-package",
        "project status missing workspace package",
        &output,
        1,
    )
    .unwrap();
    expect_empty_stdout(
        "project-status-missing-package",
        "project status missing workspace package",
        &stdout,
    )
    .unwrap();
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stderr_contains(
        "project-status-missing-package",
        "project status missing workspace package",
        &normalized_stderr,
        &format!(
            "error: `ql project status` package selector matched no workspace members under `{}`",
            project_root.to_string_lossy().replace('\\', "/")
        ),
    )
    .unwrap();
    expect_stderr_contains(
        "project-status-missing-package",
        "project status missing workspace package",
        &stderr,
        "note: selector: package `missing`",
    )
    .unwrap();
    expect_stderr_contains(
        "project-status-missing-package",
        "project status missing workspace package",
        &normalized_stderr,
        &format!(
            "hint: rerun `ql project status {}` to inspect all workspace members, or adjust `--package`",
            project_root.to_string_lossy().replace('\\', "/")
        ),
    )
    .unwrap();
}

#[test]
fn project_status_json_reports_missing_workspace_package_selector() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-status-missing-package-json");
    let project_root = write_status_workspace(&temp);

    let mut command = ql_command(&workspace_root);
    command.args([
        "project",
        "status",
        &project_root.to_string_lossy(),
        "--package",
        "missing",
        "--json",
    ]);
    let output = run_command_capture(
        &mut command,
        "`ql project status --package --json` missing workspace package",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-status-missing-package-json",
        "project status missing workspace package json",
        &output,
        1,
    )
    .unwrap();
    expect_empty_stderr(
        "project-status-missing-package-json",
        "project status missing workspace package json",
        &stderr,
    )
    .unwrap();

    let json = parse_json_output("project-status-missing-package-json", &stdout);
    assert_eq!(json["schema"], "ql.project.status.v1");
    assert_eq!(json["status"], "failed");
    assert_eq!(json["kind"], "workspace");
    assert_eq!(json["members"], serde_json::json!([]));
    assert_eq!(json["failure"]["kind"], "selection");
    let failure = &json["failure"]["selection_failure"];
    assert_eq!(failure["stage"], "package-selection");
    assert_eq!(failure["selector"], "package `missing`");
    assert_eq!(failure["target_count"], 0);
    assert!(
        failure["message"]
            .as_str()
            .expect("project status json selector miss should expose a message")
            .contains("package selector matched no workspace members"),
        "project status json selector miss should describe the missing package: {json}"
    );
}

#[test]
fn project_status_package_selector_rejects_duplicate_package_names() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-status-duplicate-package-selector");
    let project_root = temp.path().join("workspace");

    temp.write(
        "workspace/qlang.toml",
        "[workspace]\nmembers = [\"packages/a\", \"packages/b\"]\n",
    );
    temp.write(
        "workspace/packages/a/qlang.toml",
        "[package]\nname = \"util\"\n",
    );
    temp.write(
        "workspace/packages/a/src/lib.ql",
        "pub fn left() -> Int {\n    return 1\n}\n",
    );
    temp.write(
        "workspace/packages/b/qlang.toml",
        "[package]\nname = \"util\"\n",
    );
    temp.write(
        "workspace/packages/b/src/lib.ql",
        "pub fn right() -> Int {\n    return 2\n}\n",
    );

    let mut command = ql_command(&workspace_root);
    command.args([
        "project",
        "status",
        &project_root.to_string_lossy(),
        "--package",
        "util",
        "--json",
    ]);
    let output = run_command_capture(
        &mut command,
        "`ql project status --package --json` duplicate workspace package",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-status-duplicate-package-selector",
        "project status duplicate workspace package selector",
        &output,
        1,
    )
    .unwrap();
    expect_empty_stderr(
        "project-status-duplicate-package-selector",
        "project status duplicate workspace package selector",
        &stderr,
    )
    .unwrap();
    let json = parse_json_output("project-status-duplicate-package-selector", &stdout);
    assert_eq!(json["schema"], "ql.project.status.v1");
    assert_eq!(json["status"], "failed");
    assert_eq!(json["failure"]["kind"], "selection");
    let failure = &json["failure"]["selection_failure"];
    assert_eq!(failure["stage"], "package-selection");
    assert_eq!(failure["selector"], "package `util`");
    assert_eq!(failure["target_count"], 2);
    assert!(
        failure["message"]
            .as_str()
            .expect("project status json duplicate selector should expose a message")
            .contains("contains multiple members for package `util`"),
        "project status json duplicate selector should describe ambiguous matches: {json}"
    );
}

#[test]
fn project_status_package_selector_surfaces_broken_workspace_member_metadata() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-status-broken-member");
    let project_root = temp.path().join("workspace");

    temp.write(
        "workspace/qlang.toml",
        "[workspace]\nmembers = [\"packages/app\", \"packages/broken\"]\n",
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        "[package]\nname = \"app\"\n",
    );
    temp.write(
        "workspace/packages/app/src/lib.ql",
        "pub fn ready() -> Int {\n    return 1\n}\n",
    );
    temp.write("workspace/packages/broken/qlang.toml", "[package]\n");

    let mut command = ql_command(&workspace_root);
    command.args([
        "project",
        "status",
        &project_root.to_string_lossy(),
        "--package",
        "app",
    ]);
    let output = run_command_capture(
        &mut command,
        "`ql project status --package` broken workspace member metadata",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-status-broken-member",
        "project status broken workspace member metadata",
        &output,
        1,
    )
    .unwrap();
    expect_empty_stdout(
        "project-status-broken-member",
        "project status broken workspace member metadata",
        &stdout,
    )
    .unwrap();
    expect_stderr_contains(
        "project-status-broken-member",
        "project status broken workspace member metadata",
        &stderr.replace('\\', "/"),
        "error: `ql project status` failed to inspect workspace member `packages/broken`: manifest",
    )
    .unwrap();
    expect_stderr_contains(
        "project-status-broken-member",
        "project status broken workspace member metadata",
        &stderr,
        "does not declare `[package].name`",
    )
    .unwrap();
}
