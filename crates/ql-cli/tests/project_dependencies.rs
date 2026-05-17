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

fn expected_app_dependencies_json(
    project_root: &std::path::Path,
    request_path: &std::path::Path,
) -> JsonValue {
    json!({
        "schema": "ql.project.dependencies.v1",
        "path": request_path.to_string_lossy().replace('\\', "/"),
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
    })
}

fn write_standalone_package_with_local_dependency(temp: &TempDir) -> std::path::PathBuf {
    let project_root = temp.path().join("app");
    temp.write(
        "app/qlang.toml",
        "[dependencies]\n\"vendor.core\" = \"../vendor/core\"\n\n[package]\nname = \"app\"\n",
    );
    temp.write(
        "vendor/core/qlang.toml",
        "[package]\nname = \"vendor.core\"\n",
    );
    project_root
}

fn expected_standalone_package_dependencies_json(
    temp: &TempDir,
    project_root: &std::path::Path,
    request_path: &std::path::Path,
) -> JsonValue {
    json!({
        "schema": "ql.project.dependencies.v1",
        "path": request_path.to_string_lossy().replace('\\', "/"),
        "workspace_manifest_path": project_root.join("qlang.toml").to_string_lossy().replace('\\', "/"),
        "package_name": "app",
        "dependencies": [
            {
                "kind": "local",
                "member": null,
                "dependency_path": "../vendor/core",
                "package_name": "vendor.core",
                "manifest_path": temp.path().join("vendor/core/qlang.toml").to_string_lossy().replace('\\', "/"),
            }
        ],
    })
}

fn assert_dependencies_selection_failure_json(
    case_name: &str,
    stdout: &str,
    project_root: &std::path::Path,
    request_path: &std::path::Path,
    package_name: &str,
    target_count: Option<usize>,
    message_fragment: &str,
) {
    let actual = parse_json_output(case_name, stdout);
    assert_eq!(actual["schema"], "ql.project.dependencies.v1");
    assert_eq!(
        actual["path"],
        request_path.to_string_lossy().replace('\\', "/")
    );
    assert_eq!(
        actual["workspace_manifest_path"],
        project_root
            .join("qlang.toml")
            .to_string_lossy()
            .replace('\\', "/")
    );
    assert_eq!(actual["package_name"], package_name);
    assert_eq!(actual["dependencies"], json!([]));
    assert_eq!(actual["failure"]["kind"], "selection");

    let failure = &actual["failure"]["selection_failure"];
    assert_eq!(failure["stage"], "package-selection");
    assert_eq!(failure["selector"], format!("package `{package_name}`"));
    assert_eq!(failure["target_count"], json!(target_count));
    assert!(
        failure["message"]
            .as_str()
            .expect("dependencies selector failure should include a message")
            .contains(message_fragment),
        "dependencies selector failure should describe the failure: {actual}"
    );
}

#[test]
fn project_dependencies_supports_standalone_package() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-dependencies-package");
    let project_root = write_standalone_package_with_local_dependency(&temp);

    let mut command = ql_command(&workspace_root);
    command.args(["project", "dependencies", &project_root.to_string_lossy()]);
    let output = run_command_capture(&mut command, "`ql project dependencies` standalone package");
    let (stdout, stderr) = expect_success(
        "project-dependencies-package",
        "project dependencies standalone package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-dependencies-package",
        "project dependencies standalone package",
        &stderr,
    )
    .unwrap();

    let expected = format!(
        "workspace_manifest: {}\npackage: app\ndependencies:\n  - ../vendor/core (vendor.core, local)\n",
        project_root
            .join("qlang.toml")
            .to_string_lossy()
            .replace('\\', "/")
    );
    expect_snapshot_matches(
        "project-dependencies-package",
        "project dependencies standalone package stdout",
        &expected,
        &stdout.replace('\\', "/"),
    )
    .unwrap();
}

#[test]
fn project_dependencies_supports_standalone_package_source_path() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-dependencies-package-source");
    let project_root = write_standalone_package_with_local_dependency(&temp);
    let request_path = temp.write("app/src/main.ql", "fn main() -> Int {\n    return 0\n}\n");

    let mut command = ql_command(&workspace_root);
    command.args(["project", "dependencies", &request_path.to_string_lossy()]);
    let output = run_command_capture(
        &mut command,
        "`ql project dependencies` standalone package source path",
    );
    let (stdout, stderr) = expect_success(
        "project-dependencies-package-source",
        "project dependencies standalone package source path",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-dependencies-package-source",
        "project dependencies standalone package source path",
        &stderr,
    )
    .unwrap();

    let expected = format!(
        "workspace_manifest: {}\npackage: app\ndependencies:\n  - ../vendor/core (vendor.core, local)\n",
        project_root
            .join("qlang.toml")
            .to_string_lossy()
            .replace('\\', "/")
    );
    expect_snapshot_matches(
        "project-dependencies-package-source",
        "project dependencies standalone source path stdout",
        &expected,
        &stdout.replace('\\', "/"),
    )
    .unwrap();
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
    let expected = expected_app_dependencies_json(&project_root, &project_root);
    assert_eq!(actual, expected, "project dependencies json stdout");
}

#[test]
fn project_dependencies_json_supports_standalone_package() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-dependencies-package-json");
    let project_root = write_standalone_package_with_local_dependency(&temp);

    let mut command = ql_command(&workspace_root);
    command.args([
        "project",
        "dependencies",
        &project_root.to_string_lossy(),
        "--json",
    ]);
    let output = run_command_capture(
        &mut command,
        "`ql project dependencies --json` standalone package",
    );
    let (stdout, stderr) = expect_success(
        "project-dependencies-package-json",
        "project dependencies standalone package json",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-dependencies-package-json",
        "project dependencies standalone package json",
        &stderr,
    )
    .unwrap();

    let actual = parse_json_output("project-dependencies-package-json", &stdout);
    let expected =
        expected_standalone_package_dependencies_json(&temp, &project_root, &project_root);
    assert_eq!(
        actual, expected,
        "project dependencies standalone package json stdout"
    );
}

#[test]
fn project_dependencies_json_supports_standalone_package_source_path() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-dependencies-package-source-json");
    let project_root = write_standalone_package_with_local_dependency(&temp);
    let request_path = temp.write("app/src/main.ql", "fn main() -> Int {\n    return 0\n}\n");

    let mut command = ql_command(&workspace_root);
    command.args([
        "project",
        "dependencies",
        &request_path.to_string_lossy(),
        "--json",
    ]);
    let output = run_command_capture(
        &mut command,
        "`ql project dependencies --json` standalone package source path",
    );
    let (stdout, stderr) = expect_success(
        "project-dependencies-package-source-json",
        "project dependencies standalone package source path json",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-dependencies-package-source-json",
        "project dependencies standalone package source path json",
        &stderr,
    )
    .unwrap();

    let actual = parse_json_output("project-dependencies-package-source-json", &stdout);
    let expected =
        expected_standalone_package_dependencies_json(&temp, &project_root, &request_path);
    assert_eq!(
        actual, expected,
        "project dependencies standalone package source path json stdout"
    );
}

#[test]
fn project_dependencies_json_derives_workspace_member_package_name_from_member_directory() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-dependencies-derived-name-json");
    let project_root = write_workspace_with_app_dependencies(&temp);
    let request_path = project_root.join("packages/app");

    let mut command = ql_command(&workspace_root);
    command.args([
        "project",
        "dependencies",
        &request_path.to_string_lossy(),
        "--json",
    ]);
    let output = run_command_capture(
        &mut command,
        "`ql project dependencies --json` derived workspace member package name",
    );
    let (stdout, stderr) = expect_success(
        "project-dependencies-derived-name-json",
        "derive workspace member package name for dependencies json",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-dependencies-derived-name-json",
        "derive workspace member package name for dependencies json",
        &stderr,
    )
    .unwrap();

    let actual = parse_json_output("project-dependencies-derived-name-json", &stdout);
    let expected = expected_app_dependencies_json(&project_root, &request_path);
    assert_eq!(
        actual, expected,
        "project dependencies derived-name json stdout"
    );
}

#[test]
fn project_dependencies_json_derives_workspace_member_package_name_from_member_source_path() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-dependencies-derived-source-json");
    let project_root = write_workspace_with_app_dependencies(&temp);
    let request_path = project_root.join("packages/app/src/main.ql");

    let mut command = ql_command(&workspace_root);
    command.args([
        "project",
        "dependencies",
        &request_path.to_string_lossy(),
        "--json",
    ]);
    let output = run_command_capture(
        &mut command,
        "`ql project dependencies --json` derived workspace member source package name",
    );
    let (stdout, stderr) = expect_success(
        "project-dependencies-derived-source-json",
        "derive workspace member source package name for dependencies json",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-dependencies-derived-source-json",
        "derive workspace member source package name for dependencies json",
        &stderr,
    )
    .unwrap();

    let actual = parse_json_output("project-dependencies-derived-source-json", &stdout);
    let expected = expected_app_dependencies_json(&project_root, &request_path);
    assert_eq!(
        actual, expected,
        "project dependencies source-path derived-name json stdout"
    );
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
            "error: `ql project dependencies` package selector matched no workspace members under `{}`",
            project_root.to_string_lossy().replace('\\', "/")
        ),
    )
    .unwrap();
    expect_stderr_contains(
        "project-dependencies-missing",
        "project dependencies missing package",
        &stderr,
        "note: selector: package `missing`",
    )
    .unwrap();
    expect_stderr_contains(
        "project-dependencies-missing",
        "project dependencies missing package",
        &stderr.replace('\\', "/"),
        &format!(
            "hint: rerun `ql project dependencies {}` to inspect all workspace members, or adjust `--name`",
            project_root.to_string_lossy().replace('\\', "/")
        ),
    )
    .unwrap();
}

#[test]
fn project_dependencies_json_reports_missing_workspace_package() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-dependencies-missing-json");
    let project_root = write_workspace_with_app_dependencies(&temp);

    let mut command = ql_command(&workspace_root);
    command.args([
        "project",
        "dependencies",
        &project_root.to_string_lossy(),
        "--name",
        "missing",
        "--json",
    ]);
    let output = run_command_capture(
        &mut command,
        "`ql project dependencies --json` missing package",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-dependencies-missing-json",
        "project dependencies missing package json",
        &output,
        1,
    )
    .unwrap();
    expect_empty_stderr(
        "project-dependencies-missing-json",
        "project dependencies missing package json",
        &stderr,
    )
    .unwrap();

    assert_dependencies_selection_failure_json(
        "project-dependencies-missing-json",
        &stdout,
        &project_root,
        &project_root,
        "missing",
        Some(0),
        "package selector matched no workspace members",
    );
}

#[test]
fn project_dependencies_reject_duplicate_workspace_package_names_for_name_selector() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-dependencies-duplicate");
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
        "dependencies",
        &project_root.to_string_lossy(),
        "--name",
        "util",
    ]);
    let output = run_command_capture(
        &mut command,
        "`ql project dependencies` duplicate workspace package",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-dependencies-duplicate",
        "project dependencies duplicate package",
        &output,
        1,
    )
    .unwrap();
    expect_empty_stdout(
        "project-dependencies-duplicate",
        "project dependencies duplicate package",
        &stdout,
    )
    .unwrap();
    expect_stderr_contains(
        "project-dependencies-duplicate",
        "project dependencies duplicate package",
        &stderr.replace('\\', "/"),
        &format!(
            "error: `ql project dependencies` workspace manifest `{}` contains multiple members for package `util`: packages/a ({}/packages/a/qlang.toml), packages/b ({}/packages/b/qlang.toml)",
            project_root.join("qlang.toml").to_string_lossy().replace('\\', "/"),
            project_root.to_string_lossy().replace('\\', "/"),
            project_root.to_string_lossy().replace('\\', "/")
        ),
    )
    .unwrap();
}

#[test]
fn project_dependencies_json_reports_duplicate_workspace_package_names() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-dependencies-duplicate-json");
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
        "dependencies",
        &project_root.to_string_lossy(),
        "--name",
        "util",
        "--json",
    ]);
    let output = run_command_capture(
        &mut command,
        "`ql project dependencies --json` duplicate workspace package",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-dependencies-duplicate-json",
        "project dependencies duplicate package json",
        &output,
        1,
    )
    .unwrap();
    expect_empty_stderr(
        "project-dependencies-duplicate-json",
        "project dependencies duplicate package json",
        &stderr,
    )
    .unwrap();

    assert_dependencies_selection_failure_json(
        "project-dependencies-duplicate-json",
        &stdout,
        &project_root,
        &project_root,
        "util",
        Some(2),
        "contains multiple members for package `util`",
    );
}

#[test]
fn project_dependencies_surface_broken_workspace_member_metadata_for_name_selector() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-dependencies-broken-member");
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
        "workspace/packages/app/src/main.ql",
        "fn main() -> Int {\n    return 0\n}\n",
    );
    temp.write("workspace/packages/broken/qlang.toml", "[package]\n");

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
        "`ql project dependencies` broken workspace member metadata",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-dependencies-broken-member",
        "project dependencies broken member metadata",
        &output,
        1,
    )
    .unwrap();
    expect_empty_stdout(
        "project-dependencies-broken-member",
        "project dependencies broken member metadata",
        &stdout,
    )
    .unwrap();
    expect_stderr_contains(
        "project-dependencies-broken-member",
        "project dependencies broken member metadata",
        &stderr.replace('\\', "/"),
        "error: `ql project dependencies` failed to inspect workspace member `packages/broken`: manifest",
    )
    .unwrap();
    expect_stderr_contains(
        "project-dependencies-broken-member",
        "project dependencies broken member metadata",
        &stderr,
        "does not declare `[package].name`",
    )
    .unwrap();
}

#[test]
fn project_dependencies_json_reports_broken_workspace_member_metadata() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-dependencies-broken-member-json");
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
        "workspace/packages/app/src/main.ql",
        "fn main() -> Int {\n    return 0\n}\n",
    );
    temp.write("workspace/packages/broken/qlang.toml", "[package]\n");

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
        "`ql project dependencies --json` broken workspace member metadata",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-dependencies-broken-member-json",
        "project dependencies broken member metadata json",
        &output,
        1,
    )
    .unwrap();
    expect_empty_stderr(
        "project-dependencies-broken-member-json",
        "project dependencies broken member metadata json",
        &stderr,
    )
    .unwrap();

    assert_dependencies_selection_failure_json(
        "project-dependencies-broken-member-json",
        &stdout,
        &project_root,
        &project_root,
        "app",
        None,
        "failed to inspect workspace member `packages/broken`",
    );
}
