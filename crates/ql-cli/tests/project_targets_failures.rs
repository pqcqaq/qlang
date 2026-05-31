mod support;

use serde_json::Value as JsonValue;
use std::path::PathBuf;
use support::{
    TempDir, expect_empty_stderr, expect_empty_stdout, expect_exit_code, expect_stderr_contains,
    expect_stdout_contains_all, ql_command, run_command_capture, workspace_root,
};

fn normalize_output_text(text: &str) -> String {
    text.replace("\r\n", "\n")
}

fn parse_json_output(case_name: &str, stdout: &str) -> JsonValue {
    serde_json::from_str(&normalize_output_text(stdout))
        .unwrap_or_else(|error| panic!("[{case_name}] parse json stdout: {error}\n{stdout}"))
}

struct WorkspaceTargetsSelectorFixture {
    _temp: TempDir,
    project_root: PathBuf,
}

fn write_workspace_targets_selector_fixture(prefix: &str) -> WorkspaceTargetsSelectorFixture {
    let temp = TempDir::new(prefix);
    let project_root = temp.path().join("workspace");
    std::fs::create_dir_all(project_root.join("packages/app/src"))
        .expect("create app package source tree");
    std::fs::create_dir_all(project_root.join("packages/worker/src"))
        .expect("create worker package source tree");

    temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace/packages/worker/qlang.toml",
        r#"
[package]
name = "worker"
"#,
    );
    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/worker"]
"#,
    );
    temp.write(
        "workspace/packages/app/src/lib.ql",
        "pub fn run() -> Int { return 0 }\n",
    );
    temp.write(
        "workspace/packages/worker/src/job.ql",
        "pub fn run() -> Int { return 1 }\n",
    );

    WorkspaceTargetsSelectorFixture {
        _temp: temp,
        project_root,
    }
}

#[test]
fn project_targets_rejects_workspace_member_dependency_cycles() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-targets-workspace-dependency-cycle");
    let project_root = temp.path().join("workspace");
    std::fs::create_dir_all(project_root.join("packages/app/src"))
        .expect("create app package source tree");
    std::fs::create_dir_all(project_root.join("packages/core/src"))
        .expect("create core package source tree");

    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"

[dependencies]
core = "../core"
"#,
    );
    temp.write(
        "workspace/packages/core/qlang.toml",
        r#"
[package]
name = "core"

[dependencies]
app = "../app"
"#,
    );
    temp.write(
        "workspace/packages/app/src/main.ql",
        "fn main() -> Int { return 0 }\n",
    );
    temp.write(
        "workspace/packages/core/src/lib.ql",
        "pub fn answer() -> Int { return 42 }\n",
    );

    let mut command = ql_command(&workspace_root);
    command.args(["project", "targets"]).arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql project targets` workspace dependency cycle",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-targets-workspace-dependency-cycle",
        "workspace target dependency cycle rejection",
        &output,
        1,
    )
    .expect("workspace target discovery should reject cyclic local dependencies");
    assert!(
        stdout.trim().is_empty(),
        "cyclic workspace target discovery should not print stdout, got:\n{stdout}"
    );
    assert!(
        stderr.contains("workspace member local dependencies contain a cycle involving: app, core")
            || stderr.contains(
                "workspace member local dependencies contain a cycle involving: core, app"
            ),
        "expected cycle diagnostic for workspace member local dependencies, got:\n{stderr}"
    );
}

#[test]
fn project_targets_rejects_selectors_that_match_no_targets() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-targets-selector-miss");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src/bin/tools"))
        .expect("create package source tree for selector miss test");

    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write("app/src/lib.ql", "pub fn util() -> Int { return 1 }\n");
    temp.write("app/src/main.ql", "fn main() -> Int { return 0 }\n");

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "targets", "--bin", "missing"])
        .arg(&project_root);
    let output = run_command_capture(&mut command, "`ql project targets --bin` selector miss");
    let (stdout, stderr) = expect_exit_code(
        "project-targets-selector-miss",
        "project target selector miss",
        &output,
        1,
    )
    .expect("project target selector miss should fail");
    expect_empty_stdout(
        "project-targets-selector-miss",
        "project target selector miss",
        &stdout,
    )
    .expect("project target selector miss should not print stdout");
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stderr_contains(
        "project-targets-selector-miss",
        "project target selector miss",
        &normalized_stderr,
        &format!(
            "error: `ql project targets` target selector matched no build targets under `{}`",
            project_root.to_string_lossy().replace('\\', "/")
        ),
    )
    .expect("project target selector miss should report the selector failure");
    expect_stderr_contains(
        "project-targets-selector-miss",
        "project target selector miss",
        &normalized_stderr,
        "note: selector: binary `missing`",
    )
    .expect("project target selector miss should print the selector description");
}

#[test]
fn project_targets_json_reports_selectors_that_match_no_targets() {
    let workspace_root = workspace_root();
    let fixture = write_workspace_targets_selector_fixture("ql-project-targets-selector-miss-json");

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "targets", "--bin", "missing", "--json"])
        .arg(&fixture.project_root);
    let output = run_command_capture(
        &mut command,
        "`ql project targets --bin --json` selector miss",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-targets-selector-miss-json",
        "project target selector miss json",
        &output,
        1,
    )
    .expect("project target selector miss json should fail");
    expect_empty_stderr(
        "project-targets-selector-miss-json",
        "project target selector miss json",
        &stderr,
    )
    .expect("project target selector miss json should stay on stdout");

    let json = parse_json_output("project-targets-selector-miss-json", &stdout);
    assert_eq!(json["schema"], "ql.project.targets.v1");
    assert_eq!(json["members"], serde_json::json!([]));
    assert_eq!(json["failure"]["kind"], "selection");
    let failure = &json["failure"]["selection_failure"];
    assert_eq!(failure["stage"], "target-selection");
    assert_eq!(failure["selector"], "binary `missing`");
    assert_eq!(failure["target_count"], 2);
    assert!(
        failure["message"]
            .as_str()
            .expect("project targets json selector miss should expose a message")
            .contains("target selector matched no build targets"),
        "project targets json selector miss should describe the selector miss: {json}"
    );
}

#[test]
fn project_targets_json_reports_invalid_manifest_load_failure() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-targets-invalid-manifest-json");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(&project_root)
        .expect("create project directory for invalid manifest targets json test");
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
        "targets",
        &project_root.to_string_lossy(),
        "--json",
    ]);
    let output = run_command_capture(&mut command, "`ql project targets --json` invalid manifest");
    let (stdout, stderr) = expect_exit_code(
        "project-targets-invalid-manifest-json",
        "project targets invalid manifest json",
        &output,
        1,
    )
    .expect("project targets json should fail when the manifest is syntactically invalid");
    expect_empty_stderr(
        "project-targets-invalid-manifest-json",
        "project targets invalid manifest json",
        &stderr,
    )
    .expect("project targets json invalid manifest should stay on stdout");

    let json = parse_json_output("project-targets-invalid-manifest-json", &stdout);
    assert_eq!(json["schema"], "ql.project.targets.v1");
    assert_eq!(
        json["path"],
        project_root.to_string_lossy().replace('\\', "/")
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
            .expect("project targets json invalid manifest should expose a message")
            .contains("invalid manifest"),
        "project targets json invalid manifest should describe the load failure: {json}"
    );
}

#[test]
fn project_targets_reports_no_discovered_targets() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-targets-empty");
    let project_root = temp.path().join("empty");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create package source tree for empty targets test");
    temp.write(
        "empty/qlang.toml",
        r#"
[package]
name = "empty"
"#,
    );
    temp.write("empty/src/a.ql", "pub fn a() -> Int { return 1 }\n");
    temp.write("empty/src/b.ql", "pub fn b() -> Int { return 2 }\n");

    let mut command = ql_command(&workspace_root);
    command.args(["project", "targets"]).arg(&project_root);
    let output = run_command_capture(&mut command, "`ql project targets` no-target package");
    let (stdout, stderr) = expect_exit_code(
        "project-targets-empty",
        "no-target package discovery",
        &output,
        0,
    )
    .expect("no-target package discovery should succeed");
    expect_empty_stderr(
        "project-targets-empty",
        "no-target package discovery",
        &stderr,
    )
    .expect("no-target package discovery should not print stderr");
    expect_stdout_contains_all(
        "project-targets-empty",
        &stdout.replace('\\', "/"),
        &["package: empty", "targets: (none found)"],
    )
    .expect("no-target package discovery should report explicit empty target set");
}

#[test]
fn project_targets_rejects_extra_argument() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-targets-extra-arg");
    let project_root = temp.path().join("project");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create package source tree for extra-arg targets test");
    temp.write(
        "project/qlang.toml",
        r#"
[package]
name = "project"
"#,
    );

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "targets"])
        .arg(&project_root)
        .arg("unexpected");
    let output = run_command_capture(&mut command, "`ql project targets` extra argument");
    let (stdout, stderr) = expect_exit_code(
        "project-targets-extra-arg",
        "extra argument rejection",
        &output,
        1,
    )
    .expect("`ql project targets` should fail when receiving an extra argument");
    assert!(
        stdout.trim().is_empty(),
        "expected no stdout for extra argument rejection, got:\n{stdout}"
    );
    assert!(
        stderr.contains("error: unknown `ql project targets` argument `unexpected`"),
        "expected unknown argument message, got:\n{stderr}"
    );
}
