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
    expect_empty_stdout(
        "project-status-duplicate-package-selector",
        "project status duplicate workspace package selector",
        &stdout,
    )
    .unwrap();
    expect_stderr_contains(
        "project-status-duplicate-package-selector",
        "project status duplicate workspace package selector",
        &stderr.replace('\\', "/"),
        &format!(
            "error: `ql project status` workspace manifest `{}` contains multiple members for package `util`: packages/a ({}/packages/a/qlang.toml), packages/b ({}/packages/b/qlang.toml)",
            project_root.join("qlang.toml").to_string_lossy().replace('\\', "/"),
            project_root.to_string_lossy().replace('\\', "/"),
            project_root.to_string_lossy().replace('\\', "/")
        ),
    )
    .unwrap();
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
