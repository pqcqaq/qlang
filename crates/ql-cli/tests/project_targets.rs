mod support;

use support::{
    TempDir, expect_empty_stderr, expect_exit_code, expect_snapshot_matches,
    expect_stdout_contains_all, expect_success, ql_command, run_command_capture, workspace_root,
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
fn project_targets_uses_declared_target_paths_and_ignores_default_conventions() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-targets-declared-targets");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src/runtime"))
        .expect("create package runtime source tree");
    std::fs::create_dir_all(project_root.join("src/tools"))
        .expect("create package tools source tree");

    let manifest_path = temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"

[lib]
path = "src/runtime/core.ql"

[[bin]]
path = "src/tools/repl.ql"
"#,
    );
    temp.write(
        "app/src/runtime/core.ql",
        "pub fn core() -> Int { return 1 }\n",
    );
    temp.write("app/src/tools/repl.ql", "fn main() -> Int { return 0 }\n");
    temp.write(
        "app/src/lib.ql",
        "pub fn default_lib() -> Int { return 2 }\n",
    );
    temp.write("app/src/main.ql", "fn main() -> Int { return 3 }\n");

    let mut command = ql_command(&workspace_root);
    command.args(["project", "targets"]).arg(&project_root);
    let output = run_command_capture(&mut command, "`ql project targets` declared targets");
    let (stdout, stderr) = expect_success(
        "project-targets-declared-targets",
        "declared target discovery",
        &output,
    )
    .expect("declared target discovery should succeed");
    expect_empty_stderr(
        "project-targets-declared-targets",
        "declared target discovery",
        &stderr,
    )
    .expect("declared target discovery should not print stderr");

    let normalized_stdout = stdout.replace('\\', "/");
    let manifest_display = manifest_path.to_string_lossy().replace('\\', "/");
    expect_stdout_contains_all(
        "project-targets-declared-targets",
        &normalized_stdout,
        &[
            &format!("manifest: {manifest_display}"),
            "package: app",
            "targets:",
            "  - lib: src/runtime/core.ql",
            "  - bin: src/tools/repl.ql",
        ],
    )
    .expect("declared target discovery output should include the declared lib and bin paths");
    assert!(
        !normalized_stdout.contains("src/lib.ql"),
        "declared target discovery should not fall back to the default lib target when `[lib]` is present, got:\n{stdout}"
    );
    assert!(
        !normalized_stdout.contains("src/main.ql"),
        "declared target discovery should not fall back to the default main target when `[[bin]]` is present, got:\n{stdout}"
    );
}

#[test]
fn project_targets_supports_json_output_for_declared_targets() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-targets-json-declared-targets");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src/runtime"))
        .expect("create package runtime source tree");
    std::fs::create_dir_all(project_root.join("src/tools"))
        .expect("create package tools source tree");

    let manifest_path = temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"

[lib]
path = "src/runtime/core.ql"

[[bin]]
path = "src/tools/repl.ql"
"#,
    );
    temp.write(
        "app/src/runtime/core.ql",
        "pub fn core() -> Int { return 1 }\n",
    );
    temp.write("app/src/tools/repl.ql", "fn main() -> Int { return 0 }\n");

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "targets"])
        .arg(&project_root)
        .arg("--json");
    let output = run_command_capture(&mut command, "`ql project targets --json` declared targets");
    let (stdout, stderr) = expect_success(
        "project-targets-json-declared-targets",
        "declared target json rendering",
        &output,
    )
    .expect("declared target json rendering should succeed");
    expect_empty_stderr(
        "project-targets-json-declared-targets",
        "declared target json rendering",
        &stderr,
    )
    .expect("declared target json rendering should not print stderr");

    let manifest_display = manifest_path.to_string_lossy().replace('\\', "/");
    let expected = format!(
        "{{\n  \"schema\": \"ql.project.targets.v1\",\n  \"members\": [\n    {{\n      \"manifest_path\": \"{manifest_display}\",\n      \"package_name\": \"app\",\n      \"targets\": [\n        {{\n          \"kind\": \"lib\",\n          \"path\": \"src/runtime/core.ql\"\n        }},\n        {{\n          \"kind\": \"bin\",\n          \"path\": \"src/tools/repl.ql\"\n        }}\n      ]\n    }}\n  ]\n}}\n"
    );
    expect_snapshot_matches(
        "project-targets-json-declared-targets",
        "declared target json stdout",
        &expected,
        &stdout.replace('\\', "/"),
    )
    .expect("declared target json output should match the stable contract");
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
fn project_targets_source_file_uses_workspace_root_context() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-targets-workspace-source");
    let source_path = temp
        .path()
        .join("workspace")
        .join("packages")
        .join("app")
        .join("src")
        .join("lib.ql");
    std::fs::create_dir_all(
        source_path
            .parent()
            .expect("workspace app source parent should exist"),
    )
    .expect("create app package source tree");
    std::fs::create_dir_all(
        temp.path()
            .join("workspace")
            .join("packages")
            .join("worker")
            .join("src"),
    )
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
    command.args(["project", "targets"]).arg(&source_path);
    let output = run_command_capture(
        &mut command,
        "`ql project targets` workspace member source path",
    );
    let (stdout, stderr) = expect_success(
        "project-targets-workspace-source",
        "workspace member source target discovery",
        &output,
    )
    .expect("workspace member source target discovery should succeed");
    expect_empty_stderr(
        "project-targets-workspace-source",
        "workspace member source target discovery",
        &stderr,
    )
    .expect("workspace member source target discovery should not print stderr");

    let normalized_stdout = stdout.replace('\\', "/");
    let app_manifest_display = app_manifest.to_string_lossy().replace('\\', "/");
    let worker_manifest_display = worker_manifest.to_string_lossy().replace('\\', "/");
    expect_stdout_contains_all(
        "project-targets-workspace-source",
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
    .expect("workspace member source target discovery output should include workspace targets");
}

#[test]
fn project_targets_lists_workspace_members_in_dependency_order() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-targets-workspace-dependency-order");
    let project_root = temp.path().join("workspace");
    std::fs::create_dir_all(project_root.join("packages/app/src"))
        .expect("create app package source tree");
    std::fs::create_dir_all(project_root.join("packages/core/src"))
        .expect("create core package source tree");

    let app_manifest = temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"

[dependencies]
core = "../core"
"#,
    );
    let core_manifest = temp.write(
        "workspace/packages/core/qlang.toml",
        r#"
[package]
name = "core"
"#,
    );
    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/core"]
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
        "`ql project targets` workspace dependency order",
    );
    let (stdout, stderr) = expect_success(
        "project-targets-workspace-dependency-order",
        "workspace target dependency ordering",
        &output,
    )
    .expect("workspace target discovery should succeed when members depend on one another");
    expect_empty_stderr(
        "project-targets-workspace-dependency-order",
        "workspace target dependency ordering",
        &stderr,
    )
    .expect("workspace target discovery should not print stderr");

    let normalized_stdout = stdout.replace('\\', "/");
    let app_manifest_display = app_manifest.to_string_lossy().replace('\\', "/");
    let core_manifest_display = core_manifest.to_string_lossy().replace('\\', "/");
    expect_stdout_contains_all(
        "project-targets-workspace-dependency-order",
        &normalized_stdout,
        &[
            &format!("manifest: {core_manifest_display}"),
            "package: core",
            &format!("manifest: {app_manifest_display}"),
            "package: app",
        ],
    )
    .expect("workspace target discovery should print both member packages");
    assert!(
        normalized_stdout
            .find(&format!("manifest: {core_manifest_display}"))
            .expect("core manifest should be present")
            < normalized_stdout
                .find(&format!("manifest: {app_manifest_display}"))
                .expect("app manifest should be present"),
        "workspace target discovery should list dependency members before dependents, got:\n{stdout}"
    );
}

#[test]
fn project_targets_supports_json_output_for_workspace_members_in_dependency_order() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-targets-json-workspace-dependency-order");
    let project_root = temp.path().join("workspace");
    std::fs::create_dir_all(project_root.join("packages/app/src"))
        .expect("create app package source tree");
    std::fs::create_dir_all(project_root.join("packages/core/src"))
        .expect("create core package source tree");

    let app_manifest = temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"

[dependencies]
core = "../core"
"#,
    );
    let core_manifest = temp.write(
        "workspace/packages/core/qlang.toml",
        r#"
[package]
name = "core"
"#,
    );
    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/core"]
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
    command
        .args(["project", "targets", "--json"])
        .arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql project targets --json` workspace dependency order",
    );
    let (stdout, stderr) = expect_success(
        "project-targets-json-workspace-dependency-order",
        "workspace target dependency ordering json rendering",
        &output,
    )
    .expect("workspace target json rendering should succeed when members depend on one another");
    expect_empty_stderr(
        "project-targets-json-workspace-dependency-order",
        "workspace target dependency ordering json rendering",
        &stderr,
    )
    .expect("workspace target json rendering should not print stderr");

    let app_manifest_display = app_manifest.to_string_lossy().replace('\\', "/");
    let core_manifest_display = core_manifest.to_string_lossy().replace('\\', "/");
    let expected = format!(
        "{{\n  \"schema\": \"ql.project.targets.v1\",\n  \"members\": [\n    {{\n      \"manifest_path\": \"{core_manifest_display}\",\n      \"package_name\": \"core\",\n      \"targets\": [\n        {{\n          \"kind\": \"lib\",\n          \"path\": \"src/lib.ql\"\n        }}\n      ]\n    }},\n    {{\n      \"manifest_path\": \"{app_manifest_display}\",\n      \"package_name\": \"app\",\n      \"targets\": [\n        {{\n          \"kind\": \"bin\",\n          \"path\": \"src/main.ql\"\n        }}\n      ]\n    }}\n  ]\n}}\n"
    );
    expect_snapshot_matches(
        "project-targets-json-workspace-dependency-order",
        "workspace target json stdout",
        &expected,
        &stdout.replace('\\', "/"),
    )
    .expect("workspace target json output should match the stable contract");
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
