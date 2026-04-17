mod support;

use support::{
    TempDir, expect_empty_stderr, expect_exit_code, expect_stdout_contains_all, expect_success,
    ql_command, run_command_capture, workspace_root,
};

#[test]
fn project_targets_lists_package_targets() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-targets-package");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src/bin/tools"))
        .expect("create package source tree for targets test");
    let manifest_path = temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write("app/src/lib.ql", "pub fn util() -> Int { return 1 }\n");
    temp.write("app/src/main.ql", "fn main() -> Int { return 0 }\n");
    temp.write("app/src/bin/admin.ql", "fn main() -> Int { return 1 }\n");
    temp.write(
        "app/src/bin/tools/repl.ql",
        "fn main() -> Int { return 2 }\n",
    );

    let mut command = ql_command(&workspace_root);
    command.args(["project", "targets"]).arg(&project_root);
    let output = run_command_capture(&mut command, "`ql project targets` package");
    let (stdout, stderr) = expect_success(
        "project-targets-package",
        "package target discovery",
        &output,
    )
    .expect("package target discovery should succeed");
    expect_empty_stderr(
        "project-targets-package",
        "package target discovery",
        &stderr,
    )
    .expect("package target discovery should not print stderr");

    let normalized_stdout = stdout.replace('\\', "/");
    let manifest_display = manifest_path.to_string_lossy().replace('\\', "/");
    expect_stdout_contains_all(
        "project-targets-package",
        &normalized_stdout,
        &[
            &format!("manifest: {manifest_display}"),
            "package: app",
            "targets:",
            "  - lib: src/lib.ql",
            "  - bin: src/main.ql",
            "  - bin: src/bin/admin.ql",
            "  - bin: src/bin/tools/repl.ql",
        ],
    )
    .expect("package target discovery output should include all discovered targets");
}

#[test]
fn project_targets_lists_workspace_member_targets() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-targets-workspace");
    let project_root = temp.path().join("workspace");
    std::fs::create_dir_all(project_root.join("packages/app/src"))
        .expect("create app package source tree");
    std::fs::create_dir_all(project_root.join("packages/worker/src"))
        .expect("create worker package source tree");

    let app_manifest = temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    let worker_manifest = temp.write(
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

    let mut command = ql_command(&workspace_root);
    command.args(["project", "targets"]).arg(&project_root);
    let output = run_command_capture(&mut command, "`ql project targets` workspace");
    let (stdout, stderr) = expect_success(
        "project-targets-workspace",
        "workspace target discovery",
        &output,
    )
    .expect("workspace target discovery should succeed");
    expect_empty_stderr(
        "project-targets-workspace",
        "workspace target discovery",
        &stderr,
    )
    .expect("workspace target discovery should not print stderr");

    let normalized_stdout = stdout.replace('\\', "/");
    let app_manifest_display = app_manifest.to_string_lossy().replace('\\', "/");
    let worker_manifest_display = worker_manifest.to_string_lossy().replace('\\', "/");
    expect_stdout_contains_all(
        "project-targets-workspace",
        &normalized_stdout,
        &[
            &format!("manifest: {app_manifest_display}"),
            "package: app",
            "  - lib: src/lib.ql",
            &format!("manifest: {worker_manifest_display}"),
            "package: worker",
            "  - source: src/job.ql",
        ],
    )
    .expect("workspace target discovery output should include member targets");
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
    let (stdout, stderr) = expect_success(
        "project-targets-empty",
        "no-target package discovery",
        &output,
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
