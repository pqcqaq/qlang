mod support;

use serde_json::Value as JsonValue;
use support::{
    TempDir, expect_empty_stderr, expect_stdout_contains_all, expect_success, normalize,
    ql_command, run_command_capture, workspace_root,
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
    command.args(["project", "status", &project_root.to_string_lossy(), "--json"]);
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
    assert_eq!(members.len(), 2, "project status should report both members");
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
    assert_eq!(app["dependencies"][1]["dependency_path"], "../../vendor/core");

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
