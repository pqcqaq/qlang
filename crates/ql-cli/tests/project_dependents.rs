mod support;

use serde_json::{Value as JsonValue, json};
use support::{
    TempDir, expect_empty_stderr, expect_empty_stdout, expect_exit_code, expect_snapshot_matches,
    expect_stderr_contains, expect_success, normalize, ql_command, run_command_capture,
    workspace_root,
};

fn parse_json_output(case_name: &str, stdout: &str) -> JsonValue {
    serde_json::from_str(&normalize(stdout))
        .unwrap_or_else(|error| panic!("[{case_name}] parse json stdout: {error}\n{stdout}"))
}

fn write_workspace_with_core_dependents(temp: &TempDir) -> std::path::PathBuf {
    let project_root = temp.path().join("workspace");
    temp.write(
        "workspace/qlang.toml",
        "[workspace]\nmembers = [\"packages/app\", \"packages/core\", \"packages/tools\"]\n",
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        "[dependencies]\ncore = \"../core\"\n\n[package]\nname = \"app\"\n",
    );
    temp.write(
        "workspace/packages/core/qlang.toml",
        "[package]\nname = \"core\"\n",
    );
    temp.write(
        "workspace/packages/tools/qlang.toml",
        "[dependencies]\ncore = \"../core\"\n\n[package]\nname = \"tools\"\n",
    );
    temp.write(
        "workspace/packages/app/src/main.ql",
        "fn main() -> Int {\n    return 0\n}\n",
    );
    temp.write(
        "workspace/packages/core/src/main.ql",
        "fn main() -> Int {\n    return 0\n}\n",
    );
    temp.write(
        "workspace/packages/tools/src/main.ql",
        "fn main() -> Int {\n    return 0\n}\n",
    );
    project_root
}

#[test]
fn project_dependents_lists_workspace_member_dependents_from_member_source_path() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-dependents-success");
    let project_root = write_workspace_with_core_dependents(&temp);
    let request_path = project_root.join("packages/core/src/main.ql");

    let mut command = ql_command(&workspace_root);
    command.args([
        "project",
        "dependents",
        &request_path.to_string_lossy(),
        "--name",
        "core",
    ]);
    let output = run_command_capture(
        &mut command,
        "`ql project dependents` workspace member source path",
    );
    let (stdout, stderr) = expect_success(
        "project-dependents-success",
        "list workspace member dependents",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-dependents-success",
        "list workspace member dependents",
        &stderr,
    )
    .unwrap();

    let expected = format!(
        "workspace_manifest: {}\npackage: core\ndependents:\n  - packages/app (app)\n  - packages/tools (tools)\n",
        project_root
            .join("qlang.toml")
            .to_string_lossy()
            .replace('\\', "/")
    );
    expect_snapshot_matches(
        "project-dependents-success",
        "project dependents stdout",
        &expected,
        &stdout.replace('\\', "/"),
    )
    .unwrap();
}

#[test]
fn project_dependents_supports_json_output() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-dependents-json");
    let project_root = write_workspace_with_core_dependents(&temp);

    let mut command = ql_command(&workspace_root);
    command.args([
        "project",
        "dependents",
        &project_root.to_string_lossy(),
        "--name",
        "app",
        "--json",
    ]);
    let output = run_command_capture(&mut command, "`ql project dependents --json` workspace");
    let (stdout, stderr) = expect_success(
        "project-dependents-json",
        "project dependents json rendering",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-dependents-json",
        "project dependents json rendering",
        &stderr,
    )
    .unwrap();

    let actual = parse_json_output("project-dependents-json", &stdout);
    let expected = json!({
        "schema": "ql.project.dependents.v1",
        "path": project_root.to_string_lossy().replace('\\', "/"),
        "workspace_manifest_path": project_root.join("qlang.toml").to_string_lossy().replace('\\', "/"),
        "package_name": "app",
        "dependents": [],
    });
    assert_eq!(actual, expected, "project dependents json stdout");
}

#[test]
fn project_dependents_refuses_missing_workspace_package() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-dependents-missing");
    let project_root = write_workspace_with_core_dependents(&temp);

    let mut command = ql_command(&workspace_root);
    command.args([
        "project",
        "dependents",
        &project_root.to_string_lossy(),
        "--name",
        "missing",
    ]);
    let output = run_command_capture(&mut command, "`ql project dependents` missing package");
    let (stdout, stderr) = expect_exit_code(
        "project-dependents-missing",
        "project dependents missing package",
        &output,
        1,
    )
    .unwrap();
    expect_empty_stdout(
        "project-dependents-missing",
        "project dependents missing package",
        &stdout,
    )
    .unwrap();
    expect_stderr_contains(
        "project-dependents-missing",
        "project dependents missing package",
        &stderr.replace('\\', "/"),
        &format!(
            "error: `ql project dependents` workspace manifest `{}` does not contain package `missing`",
            project_root.join("qlang.toml").to_string_lossy().replace('\\', "/")
        ),
    )
    .unwrap();
}
