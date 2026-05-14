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
fn project_dependents_derives_workspace_member_package_name_from_member_source_path() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-dependents-derived-name");
    let project_root = write_workspace_with_core_dependents(&temp);
    let request_path = project_root.join("packages/core/src/main.ql");

    let mut command = ql_command(&workspace_root);
    command.args(["project", "dependents", &request_path.to_string_lossy()]);
    let output = run_command_capture(
        &mut command,
        "`ql project dependents` derived workspace member package name",
    );
    let (stdout, stderr) = expect_success(
        "project-dependents-derived-name",
        "derive workspace member package name for dependents",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-dependents-derived-name",
        "derive workspace member package name for dependents",
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
        "project-dependents-derived-name",
        "project dependents derived-name stdout",
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
fn project_dependents_json_derives_workspace_member_package_name() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-dependents-derived-name-json");
    let project_root = write_workspace_with_core_dependents(&temp);
    let request_path = project_root.join("packages/core/src/main.ql");

    let mut command = ql_command(&workspace_root);
    command.args([
        "project",
        "dependents",
        &request_path.to_string_lossy(),
        "--json",
    ]);
    let output = run_command_capture(
        &mut command,
        "`ql project dependents --json` derived workspace member package name",
    );
    let (stdout, stderr) = expect_success(
        "project-dependents-derived-name-json",
        "derive workspace member package name for dependents json",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-dependents-derived-name-json",
        "derive workspace member package name for dependents json",
        &stderr,
    )
    .unwrap();

    let actual = parse_json_output("project-dependents-derived-name-json", &stdout);
    let expected = json!({
        "schema": "ql.project.dependents.v1",
        "path": request_path.to_string_lossy().replace('\\', "/"),
        "workspace_manifest_path": project_root.join("qlang.toml").to_string_lossy().replace('\\', "/"),
        "package_name": "core",
        "dependents": [
            {
                "member": "packages/app",
                "package_name": "app",
                "manifest_path": project_root.join("packages/app/qlang.toml").to_string_lossy().replace('\\', "/"),
            },
            {
                "member": "packages/tools",
                "package_name": "tools",
                "manifest_path": project_root.join("packages/tools/qlang.toml").to_string_lossy().replace('\\', "/"),
            }
        ],
    });
    assert_eq!(
        actual, expected,
        "project dependents derived-name json stdout"
    );
}

#[test]
fn project_dependents_requires_name_when_workspace_root_is_ambiguous() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-dependents-derived-name-missing");
    let project_root = write_workspace_with_core_dependents(&temp);

    let mut command = ql_command(&workspace_root);
    command.args(["project", "dependents", &project_root.to_string_lossy()]);
    let output = run_command_capture(
        &mut command,
        "`ql project dependents` ambiguous workspace root package name",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-dependents-derived-name-missing",
        "derive workspace member package name for dependents from workspace root",
        &output,
        1,
    )
    .unwrap();
    expect_empty_stdout(
        "project-dependents-derived-name-missing",
        "derive workspace member package name for dependents from workspace root",
        &stdout,
    )
    .unwrap();
    expect_stderr_contains(
        "project-dependents-derived-name-missing",
        "derive workspace member package name for dependents from workspace root",
        &stderr.replace('\\', "/"),
        &format!(
            "error: `ql project dependents` could not derive a package name from `{}`; rerun with `--name <package>`",
            project_root.to_string_lossy().replace('\\', "/")
        ),
    )
    .unwrap();
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
            "error: `ql project dependents` package selector matched no workspace members under `{}`",
            project_root.to_string_lossy().replace('\\', "/")
        ),
    )
    .unwrap();
    expect_stderr_contains(
        "project-dependents-missing",
        "project dependents missing package",
        &stderr,
        "note: selector: package `missing`",
    )
    .unwrap();
    expect_stderr_contains(
        "project-dependents-missing",
        "project dependents missing package",
        &stderr.replace('\\', "/"),
        &format!(
            "hint: rerun `ql project dependents {}` to inspect all workspace members, or adjust `--name`",
            project_root.to_string_lossy().replace('\\', "/")
        ),
    )
    .unwrap();
}

#[test]
fn project_dependents_reject_duplicate_workspace_package_names_for_name_selector() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-dependents-duplicate");
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
        "workspace/packages/b/qlang.toml",
        "[package]\nname = \"util\"\n",
    );

    let mut command = ql_command(&workspace_root);
    command.args([
        "project",
        "dependents",
        &project_root.to_string_lossy(),
        "--name",
        "util",
    ]);
    let output = run_command_capture(
        &mut command,
        "`ql project dependents` duplicate workspace package",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-dependents-duplicate",
        "project dependents duplicate package",
        &output,
        1,
    )
    .unwrap();
    expect_empty_stdout(
        "project-dependents-duplicate",
        "project dependents duplicate package",
        &stdout,
    )
    .unwrap();
    expect_stderr_contains(
        "project-dependents-duplicate",
        "project dependents duplicate package",
        &stderr.replace('\\', "/"),
        &format!(
            "error: `ql project dependents` workspace manifest `{}` contains multiple members for package `util`: packages/a ({}/packages/a/qlang.toml), packages/b ({}/packages/b/qlang.toml)",
            project_root.join("qlang.toml").to_string_lossy().replace('\\', "/"),
            project_root.to_string_lossy().replace('\\', "/"),
            project_root.to_string_lossy().replace('\\', "/")
        ),
    )
    .unwrap();
}

#[test]
fn project_dependents_surface_broken_workspace_member_metadata_for_name_selector() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-dependents-broken-member");
    let project_root = temp.path().join("workspace");
    temp.write(
        "workspace/qlang.toml",
        "[workspace]\nmembers = [\"packages/core\", \"packages/broken\"]\n",
    );
    temp.write(
        "workspace/packages/core/qlang.toml",
        "[package]\nname = \"core\"\n",
    );
    temp.write(
        "workspace/packages/core/src/main.ql",
        "fn main() -> Int {\n    return 0\n}\n",
    );
    temp.write("workspace/packages/broken/qlang.toml", "[package]\n");

    let mut command = ql_command(&workspace_root);
    command.args([
        "project",
        "dependents",
        &project_root.to_string_lossy(),
        "--name",
        "core",
    ]);
    let output = run_command_capture(
        &mut command,
        "`ql project dependents` broken workspace member metadata",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-dependents-broken-member",
        "project dependents broken member metadata",
        &output,
        1,
    )
    .unwrap();
    expect_empty_stdout(
        "project-dependents-broken-member",
        "project dependents broken member metadata",
        &stdout,
    )
    .unwrap();
    expect_stderr_contains(
        "project-dependents-broken-member",
        "project dependents broken member metadata",
        &stderr.replace('\\', "/"),
        "error: `ql project dependents` failed to inspect workspace member `packages/broken`: manifest",
    )
    .unwrap();
    expect_stderr_contains(
        "project-dependents-broken-member",
        "project dependents broken member metadata",
        &stderr,
        "does not declare `[package].name`",
    )
    .unwrap();
}
