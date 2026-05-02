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

fn write_workspace_with_app_dependencies(temp: &TempDir) -> std::path::PathBuf {
    let project_root = temp.path().join("workspace");
    temp.write(
        "workspace/qlang.toml",
        "[workspace]\nmembers = [\"packages/app\", \"packages/core\", \"packages/tools\"]\n",
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        "[dependencies]\ncore = \"../core\"\ntools = \"../tools\"\n\n[package]\nname = \"app\"\n",
    );
    temp.write(
        "workspace/packages/core/qlang.toml",
        "[package]\nname = \"core\"\n",
    );
    temp.write(
        "workspace/packages/tools/qlang.toml",
        "[package]\nname = \"tools\"\n",
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
fn project_dependencies_lists_workspace_member_dependencies_from_member_source_path() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-dependencies-success");
    let project_root = write_workspace_with_app_dependencies(&temp);
    let request_path = project_root.join("packages/app/src/main.ql");

    let mut command = ql_command(&workspace_root);
    command.args([
        "project",
        "dependencies",
        &request_path.to_string_lossy(),
        "--name",
        "app",
    ]);
    let output = run_command_capture(
        &mut command,
        "`ql project dependencies` workspace member source path",
    );
    let (stdout, stderr) = expect_success(
        "project-dependencies-success",
        "list workspace member dependencies",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-dependencies-success",
        "list workspace member dependencies",
        &stderr,
    )
    .unwrap();

    let expected = format!(
        "workspace_manifest: {}\npackage: app\ndependencies:\n  - packages/core (core)\n  - packages/tools (tools)\n",
        project_root
            .join("qlang.toml")
            .to_string_lossy()
            .replace('\\', "/")
    );
    expect_snapshot_matches(
        "project-dependencies-success",
        "project dependencies stdout",
        &expected,
        &stdout.replace('\\', "/"),
    )
    .unwrap();
}

#[test]
fn project_dependencies_derives_workspace_member_package_name_from_member_directory() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-dependencies-derived-name");
    let project_root = write_workspace_with_app_dependencies(&temp);
    let request_path = project_root.join("packages/app");

    let mut command = ql_command(&workspace_root);
    command.args(["project", "dependencies", &request_path.to_string_lossy()]);
    let output = run_command_capture(
        &mut command,
        "`ql project dependencies` derived workspace member package name",
    );
    let (stdout, stderr) = expect_success(
        "project-dependencies-derived-name",
        "derive workspace member package name for dependencies",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-dependencies-derived-name",
        "derive workspace member package name for dependencies",
        &stderr,
    )
    .unwrap();

    let expected = format!(
        "workspace_manifest: {}\npackage: app\ndependencies:\n  - packages/core (core)\n  - packages/tools (tools)\n",
        project_root
            .join("qlang.toml")
            .to_string_lossy()
            .replace('\\', "/")
    );
    expect_snapshot_matches(
        "project-dependencies-derived-name",
        "project dependencies derived-name stdout",
        &expected,
        &stdout.replace('\\', "/"),
    )
    .unwrap();
}

#[test]
fn project_dependencies_supports_json_output() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-dependencies-json");
    let project_root = write_workspace_with_app_dependencies(&temp);

    let mut command = ql_command(&workspace_root);
    command.args([
        "project",
        "dependencies",
        &project_root.to_string_lossy(),
        "--name",
        "app",
        "--json",
    ]);
    let output = run_command_capture(&mut command, "`ql project dependencies --json` workspace");
    let (stdout, stderr) = expect_success(
        "project-dependencies-json",
        "project dependencies json rendering",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-dependencies-json",
        "project dependencies json rendering",
        &stderr,
    )
    .unwrap();

    let actual = parse_json_output("project-dependencies-json", &stdout);
    let expected = json!({
        "schema": "ql.project.dependencies.v1",
        "path": project_root.to_string_lossy().replace('\\', "/"),
        "workspace_manifest_path": project_root.join("qlang.toml").to_string_lossy().replace('\\', "/"),
        "package_name": "app",
        "dependencies": [
            {
                "kind": "workspace",
                "member": "packages/core",
                "dependency_path": "../core",
                "package_name": "core",
                "manifest_path": project_root.join("packages/core/qlang.toml").to_string_lossy().replace('\\', "/"),
            },
            {
                "kind": "workspace",
                "member": "packages/tools",
                "dependency_path": "../tools",
                "package_name": "tools",
                "manifest_path": project_root.join("packages/tools/qlang.toml").to_string_lossy().replace('\\', "/"),
            }
        ],
    });
    assert_eq!(actual, expected, "project dependencies json stdout");
}

#[test]
fn project_dependencies_lists_external_local_path_dependencies() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-dependencies-local-path");
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
        "workspace/packages/core/qlang.toml",
        "[package]\nname = \"core\"\n",
    );
    temp.write(
        "workspace/vendor/core/qlang.toml",
        "[package]\nname = \"vendor.core\"\n",
    );

    let mut command = ql_command(&workspace_root);
    command.args([
        "project",
        "dependencies",
        &project_root.to_string_lossy(),
        "--name",
        "app",
    ]);
    let output = run_command_capture(
        &mut command,
        "`ql project dependencies` external local dependency",
    );
    let (stdout, stderr) = expect_success(
        "project-dependencies-local-path",
        "list external local dependency",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-dependencies-local-path",
        "list external local dependency",
        &stderr,
    )
    .unwrap();

    let expected = format!(
        "workspace_manifest: {}\npackage: app\ndependencies:\n  - packages/core (core)\n  - ../../vendor/core (vendor.core, local)\n",
        project_root
            .join("qlang.toml")
            .to_string_lossy()
            .replace('\\', "/")
    );
    expect_snapshot_matches(
        "project-dependencies-local-path",
        "project dependencies local path stdout",
        &expected,
        &stdout.replace('\\', "/"),
    )
    .unwrap();
}

#[test]
fn project_dependencies_json_marks_external_local_path_dependencies() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-dependencies-local-path-json");
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
        "workspace/packages/core/qlang.toml",
        "[package]\nname = \"core\"\n",
    );
    temp.write(
        "workspace/vendor/core/qlang.toml",
        "[package]\nname = \"vendor.core\"\n",
    );

    let mut command = ql_command(&workspace_root);
    command.args([
        "project",
        "dependencies",
        &project_root.to_string_lossy(),
        "--name",
        "app",
        "--json",
    ]);
    let output = run_command_capture(
        &mut command,
        "`ql project dependencies --json` external local dependency",
    );
    let (stdout, stderr) = expect_success(
        "project-dependencies-local-path-json",
        "list external local dependency as json",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-dependencies-local-path-json",
        "list external local dependency as json",
        &stderr,
    )
    .unwrap();

    let actual = parse_json_output("project-dependencies-local-path-json", &stdout);
    let expected = json!({
        "schema": "ql.project.dependencies.v1",
        "path": project_root.to_string_lossy().replace('\\', "/"),
        "workspace_manifest_path": project_root.join("qlang.toml").to_string_lossy().replace('\\', "/"),
        "package_name": "app",
        "dependencies": [
            {
                "kind": "workspace",
                "member": "packages/core",
                "dependency_path": "../core",
                "package_name": "core",
                "manifest_path": project_root.join("packages/core/qlang.toml").to_string_lossy().replace('\\', "/"),
            },
            {
                "kind": "local",
                "member": null,
                "dependency_path": "../../vendor/core",
                "package_name": "vendor.core",
                "manifest_path": project_root.join("vendor/core/qlang.toml").to_string_lossy().replace('\\', "/"),
            }
        ],
    });
    assert_eq!(
        actual, expected,
        "project dependencies local path json stdout"
    );
}

#[test]
fn project_dependencies_requires_name_when_workspace_root_is_ambiguous() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-dependencies-derived-name-missing");
    let project_root = write_workspace_with_app_dependencies(&temp);

    let mut command = ql_command(&workspace_root);
    command.args(["project", "dependencies", &project_root.to_string_lossy()]);
    let output = run_command_capture(
        &mut command,
        "`ql project dependencies` ambiguous workspace root package name",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-dependencies-derived-name-missing",
        "derive workspace member package name for dependencies from workspace root",
        &output,
        1,
    )
    .unwrap();
    expect_empty_stdout(
        "project-dependencies-derived-name-missing",
        "derive workspace member package name for dependencies from workspace root",
        &stdout,
    )
    .unwrap();
    expect_stderr_contains(
        "project-dependencies-derived-name-missing",
        "derive workspace member package name for dependencies from workspace root",
        &stderr.replace('\\', "/"),
        &format!(
            "error: `ql project dependencies` could not derive a package name from `{}`; rerun with `--name <package>`",
            project_root.to_string_lossy().replace('\\', "/")
        ),
    )
    .unwrap();
}

#[test]
fn project_dependencies_refuses_missing_workspace_package() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-dependencies-missing");
    let project_root = write_workspace_with_app_dependencies(&temp);

    let mut command = ql_command(&workspace_root);
    command.args([
        "project",
        "dependencies",
        &project_root.to_string_lossy(),
        "--name",
        "missing",
    ]);
    let output = run_command_capture(&mut command, "`ql project dependencies` missing package");
    let (stdout, stderr) = expect_exit_code(
        "project-dependencies-missing",
        "project dependencies missing package",
        &output,
        1,
    )
    .unwrap();
    expect_empty_stdout(
        "project-dependencies-missing",
        "project dependencies missing package",
        &stdout,
    )
    .unwrap();
    expect_stderr_contains(
        "project-dependencies-missing",
        "project dependencies missing package",
        &stderr.replace('\\', "/"),
        &format!(
            "error: `ql project dependencies` workspace manifest `{}` does not contain package `missing`",
            project_root.join("qlang.toml").to_string_lossy().replace('\\', "/")
        ),
    )
    .unwrap();
}
