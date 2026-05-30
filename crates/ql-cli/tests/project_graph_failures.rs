mod support;

use serde_json::Value as JsonValue;
use support::{
    TempDir, expect_empty_stderr, expect_empty_stdout, expect_exit_code, expect_stderr_contains,
    expect_stderr_not_contains, ql_command, run_command_capture, workspace_root,
};

fn normalize_output_text(text: &str) -> String {
    text.replace("\r\n", "\n")
}

fn parse_json_output(case_name: &str, stdout: &str) -> JsonValue {
    serde_json::from_str(&normalize_output_text(stdout))
        .unwrap_or_else(|error| panic!("[{case_name}] parse json stdout: {error}\n{stdout}"))
}

#[test]
fn project_graph_rejects_manifest_without_package_or_workspace() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-graph-invalid");
    let project_root = temp.path().join("invalid");
    std::fs::create_dir_all(&project_root).expect("create invalid project directory");
    temp.write(
        "invalid/qlang.toml",
        r#"
[references]
packages = ["../core"]
"#,
    );

    let mut command = ql_command(&workspace_root);
    command.args(["project", "graph"]).arg(&project_root);
    let output = run_command_capture(&mut command, "`ql project graph` invalid manifest");
    let (stdout, stderr) = expect_exit_code(
        "project-graph-invalid",
        "invalid project graph rendering",
        &output,
        1,
    )
    .expect("invalid project graph rendering should fail with exit code 1");
    expect_empty_stdout(
        "project-graph-invalid",
        "invalid project graph rendering",
        &stdout,
    )
    .expect("invalid project graph rendering should not print stdout");
    expect_stderr_contains(
        "project-graph-invalid",
        "invalid project graph rendering",
        &stderr,
        "`qlang.toml` requires `[package]` or `[workspace]`",
    )
    .expect("invalid manifest diagnostic should mention the minimum section contract");
}

#[test]
fn project_graph_points_to_missing_package_context() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-graph-missing-package-context");
    let source_path = temp.write(
        "workspace/loose.ql",
        r#"
fn main() -> Int {
    return 1
}
"#,
    );

    let mut command = ql_command(&workspace_root);
    command.args(["project", "graph"]).arg(&source_path);
    let output = run_command_capture(&mut command, "`ql project graph` missing package context");
    let (stdout, stderr) = expect_exit_code(
        "project-graph-missing-package-context",
        "project graph rendering with missing package context",
        &output,
        1,
    )
    .expect("project graph should fail when the target path is outside any package/workspace");
    expect_empty_stdout(
        "project-graph-missing-package-context",
        "project graph rendering with missing package context",
        &stdout,
    )
    .expect("project graph should not print stdout when package context is missing");
    let normalized_stderr = stderr.replace('\\', "/");
    let source_display = source_path.to_string_lossy().replace('\\', "/");
    let error_line = format!(
        "error: `ql project graph` requires a package or workspace manifest; could not find `qlang.toml` starting from `{source_display}`"
    );
    let old_error_line =
        format!("error: could not find `qlang.toml` starting from `{source_display}`");
    let rerun_hint = format!(
        "hint: rerun `ql project graph {source_display}` after adding `qlang.toml` for this path"
    );
    expect_stderr_contains(
        "project-graph-missing-package-context",
        "project graph rendering with missing package context",
        &normalized_stderr,
        &error_line,
    )
    .expect("project graph should preserve the command label for missing package context");
    expect_stderr_not_contains(
        "project-graph-missing-package-context",
        "project graph rendering with missing package context",
        &normalized_stderr,
        &old_error_line,
    )
    .expect("project graph should not fall back to the unlabeled manifest-not-found error");
    expect_stderr_contains(
        "project-graph-missing-package-context",
        "project graph rendering with missing package context",
        &normalized_stderr,
        "note: `ql project graph` only renders package/workspace graphs for packages or workspace members discoverable from `qlang.toml`",
    )
    .expect("project graph should explain the package/workspace discovery contract");
    expect_stderr_contains(
        "project-graph-missing-package-context",
        "project graph rendering with missing package context",
        &normalized_stderr,
        &rerun_hint,
    )
    .expect("project graph should preserve the original target path in the rerun hint");
    expect_stderr_not_contains(
        "project-graph-missing-package-context",
        "project graph rendering with missing package context",
        &normalized_stderr,
        "note: failing package manifest:",
    )
    .expect("project graph should not pretend a package manifest was already found");
}

#[test]
fn project_graph_preserves_invalid_manifest_rerun_hint() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-graph-invalid-manifest");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(&project_root)
        .expect("create project directory for invalid manifest graph test");
    let manifest_path = temp.write(
        "workspace/app/qlang.toml",
        r#"
[package
name = "app"
"#,
    );

    let mut command = ql_command(&workspace_root);
    command.args(["project", "graph"]).arg(&project_root);
    let output = run_command_capture(&mut command, "`ql project graph` invalid manifest");
    let (stdout, stderr) = expect_exit_code(
        "project-graph-invalid-manifest",
        "project graph rendering with invalid manifest",
        &output,
        1,
    )
    .expect("project graph should fail when the manifest is syntactically invalid");
    expect_empty_stdout(
        "project-graph-invalid-manifest",
        "project graph rendering with invalid manifest",
        &stdout,
    )
    .expect("project graph should not print stdout for invalid manifests");
    let normalized_stderr = stderr.replace('\\', "/");
    let manifest_display = manifest_path.to_string_lossy().replace('\\', "/");
    let error_line = format!("error: `ql project graph` invalid manifest `{manifest_display}`");
    let old_error_line = format!("error: invalid manifest `{manifest_display}`");
    let manifest_note = format!("note: failing package manifest: {manifest_display}");
    let rerun_hint = format!(
        "hint: rerun `ql project graph {manifest_display}` after fixing the package manifest"
    );
    expect_stderr_contains(
        "project-graph-invalid-manifest",
        "project graph rendering with invalid manifest",
        &normalized_stderr,
        &error_line,
    )
    .expect("project graph should preserve the command label for invalid manifests");
    expect_stderr_not_contains(
        "project-graph-invalid-manifest",
        "project graph rendering with invalid manifest",
        &normalized_stderr,
        &old_error_line,
    )
    .expect("project graph should not fall back to the unlabeled invalid-manifest error");
    expect_stderr_contains(
        "project-graph-invalid-manifest",
        "project graph rendering with invalid manifest",
        &normalized_stderr,
        &manifest_note,
    )
    .expect("project graph should point to the failing package manifest");
    expect_stderr_contains(
        "project-graph-invalid-manifest",
        "project graph rendering with invalid manifest",
        &normalized_stderr,
        &rerun_hint,
    )
    .expect("project graph should preserve the direct rerun hint for invalid manifests");
}

#[test]
fn project_graph_json_reports_invalid_manifest_load_failure() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-graph-invalid-manifest-json");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(&project_root)
        .expect("create project directory for invalid manifest graph json test");
    let manifest_path = temp.write(
        "workspace/app/qlang.toml",
        r#"
[package
name = "app"
"#,
    );

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "graph", "--json"])
        .arg(&project_root);
    let output = run_command_capture(&mut command, "`ql project graph --json` invalid manifest");
    let (stdout, stderr) = expect_exit_code(
        "project-graph-invalid-manifest-json",
        "project graph json invalid manifest",
        &output,
        1,
    )
    .expect("project graph json should fail when the manifest is syntactically invalid");
    expect_empty_stderr(
        "project-graph-invalid-manifest-json",
        "project graph json invalid manifest",
        &stderr,
    )
    .expect("project graph json invalid manifest should stay on stdout");

    let json = parse_json_output("project-graph-invalid-manifest-json", &stdout);
    assert_eq!(json["schema"], "ql.project.graph.v1");
    assert_eq!(
        json["path"],
        project_root.to_string_lossy().replace('\\', "/")
    );
    assert_eq!(
        json["manifest_path"],
        manifest_path.to_string_lossy().replace('\\', "/")
    );
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
            .expect("project graph json invalid manifest should expose a message")
            .contains("invalid manifest"),
        "project graph json invalid manifest should describe the load failure: {json}"
    );
}

#[test]
fn project_graph_preserves_missing_package_name_error_surface() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-graph-missing-package-name");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(&project_root)
        .expect("create project directory for missing package name graph test");
    let manifest_path = temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
version = "0.1.0"
"#,
    );

    let mut command = ql_command(&workspace_root);
    command.args(["project", "graph"]).arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql project graph` missing package manifest metadata",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-graph-missing-package-name",
        "project graph rendering with missing package name",
        &output,
        1,
    )
    .expect("project graph should fail when the manifest does not declare `[package].name`");
    expect_empty_stdout(
        "project-graph-missing-package-name",
        "project graph rendering with missing package name",
        &stdout,
    )
    .expect("project graph should not print stdout when package metadata is missing");
    let normalized_stderr = stderr.replace('\\', "/");
    let manifest_display = manifest_path.to_string_lossy().replace('\\', "/");
    expect_stderr_contains(
        "project-graph-missing-package-name",
        "project graph rendering with missing package name",
        &normalized_stderr,
        &format!(
            "error: `ql project graph` manifest `{manifest_display}` does not declare `[package].name`"
        ),
    )
    .expect("project graph should preserve the command label for missing package names");
    expect_stderr_not_contains(
        "project-graph-missing-package-name",
        "project graph rendering with missing package name",
        &normalized_stderr,
        &format!("error: invalid manifest `{manifest_display}`: `[package].name` must be present"),
    )
    .expect("project graph should not fall back to the parse-error missing package-name message");
    expect_stderr_contains(
        "project-graph-missing-package-name",
        "project graph rendering with missing package name",
        &normalized_stderr,
        &format!("note: failing package manifest: {manifest_display}"),
    )
    .expect("project graph should point to the failing package manifest");
    expect_stderr_contains(
        "project-graph-missing-package-name",
        "project graph rendering with missing package name",
        &normalized_stderr,
        &format!(
            "hint: rerun `ql project graph {manifest_display}` after fixing the package manifest"
        ),
    )
    .expect("project graph should preserve the direct rerun hint for missing package names");
}
