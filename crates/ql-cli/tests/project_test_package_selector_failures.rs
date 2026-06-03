mod support;

use std::fs;

use support::{
    TempDir, expect_empty_stderr, expect_empty_stdout, expect_exit_code, expect_stderr_contains,
    expect_stdout_contains_all, expect_success, ql_command, run_command_capture, workspace_root,
};

#[test]
fn test_workspace_path_rejects_unknown_package_selector() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-package-selector-missing");
    let project_root = temp.path().join("workspace");
    fs::create_dir_all(project_root.join("packages/app/src"))
        .expect("create app package source tree");
    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app"]
"#,
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace/packages/app/src/lib.ql",
        "pub fn helper() -> Int { return 1 }\n",
    );
    temp.write(
        "workspace/packages/app/tests/smoke.ql",
        "fn main() -> Int { return 0 }\n",
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["test"])
        .arg(&project_root)
        .args(["--package", "missing"]);
    let output = run_command_capture(
        &mut command,
        "`ql test --package` unknown workspace package",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-test-package-selector-missing",
        "unknown workspace package selector",
        &output,
        1,
    )
    .expect("workspace-path `ql test --package` should reject unknown packages");
    expect_empty_stdout(
        "project-test-package-selector-missing",
        "unknown workspace package selector",
        &stdout,
    )
    .expect("unknown workspace package selector should not print stdout");
    expect_stderr_contains(
        "project-test-package-selector-missing",
        "unknown workspace package selector",
        &stderr.replace('\\', "/"),
        &format!(
            "error: `ql test` package selector matched no workspace members under `{}`",
            project_root.to_string_lossy().replace('\\', "/")
        ),
    )
    .expect("unknown workspace package selector should report the missing package");
    expect_stderr_contains(
        "project-test-package-selector-missing",
        "unknown workspace package selector",
        &stderr.replace('\\', "/"),
        &format!(
            "hint: rerun `ql test {}` to inspect all workspace members, or adjust `--package`",
            project_root.to_string_lossy().replace('\\', "/")
        ),
    )
    .expect("unknown workspace package selector should suggest inspecting discovered members");
}

#[test]
fn test_workspace_path_rejects_duplicate_package_selector() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-package-selector-duplicate");
    let project_root = temp.path().join("workspace");
    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/a", "packages/b"]
"#,
    );
    temp.write(
        "workspace/packages/a/qlang.toml",
        r#"
[package]
name = "util"
"#,
    );
    temp.write(
        "workspace/packages/a/src/lib.ql",
        "pub fn a() -> Int { return 1 }\n",
    );
    temp.write(
        "workspace/packages/a/tests/smoke.ql",
        "fn main() -> Int { return 0 }\n",
    );
    temp.write(
        "workspace/packages/b/qlang.toml",
        r#"
[package]
name = "util"
"#,
    );
    temp.write(
        "workspace/packages/b/src/lib.ql",
        "pub fn b() -> Int { return 2 }\n",
    );
    temp.write(
        "workspace/packages/b/tests/smoke.ql",
        "fn main() -> Int { return 0 }\n",
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["test"])
        .arg(&project_root)
        .args(["--package", "util"]);
    let output = run_command_capture(
        &mut command,
        "`ql test --package` duplicate workspace package",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-test-package-selector-duplicate",
        "duplicate workspace package selector",
        &output,
        1,
    )
    .expect("workspace-path `ql test --package` should reject duplicate package names");
    expect_empty_stdout(
        "project-test-package-selector-duplicate",
        "duplicate workspace package selector",
        &stdout,
    )
    .expect("duplicate workspace package selector should not print stdout");
    expect_stderr_contains(
        "project-test-package-selector-duplicate",
        "duplicate workspace package selector",
        &stderr.replace('\\', "/"),
        &format!(
            "error: `ql test` workspace manifest `{}` contains multiple members for package `util`: packages/a ({}/packages/a/qlang.toml), packages/b ({}/packages/b/qlang.toml)",
            project_root.join("qlang.toml").to_string_lossy().replace('\\', "/"),
            project_root.to_string_lossy().replace('\\', "/"),
            project_root.to_string_lossy().replace('\\', "/")
        ),
    )
    .expect("duplicate workspace package selector should list matching members");
}

#[test]
fn test_workspace_path_rejects_invalid_package_selector() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-package-selector-invalid");
    let project_root = temp.path().join("workspace");
    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app"]
"#,
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace/packages/app/src/lib.ql",
        "pub fn helper() -> Int { return 1 }\n",
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["test"])
        .arg(&project_root)
        .args(["--package", "packages/app"]);
    let output = run_command_capture(
        &mut command,
        "`ql test --package` invalid workspace package",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-test-package-selector-invalid",
        "invalid workspace package selector",
        &output,
        1,
    )
    .expect("workspace-path `ql test --package` should reject invalid package names");
    expect_empty_stdout(
        "project-test-package-selector-invalid",
        "invalid workspace package selector",
        &stdout,
    )
    .expect("invalid workspace package selector should not print stdout");
    expect_stderr_contains(
        "project-test-package-selector-invalid",
        "invalid workspace package selector",
        &stderr,
        "error: `ql test` does not accept package name `packages/app` because it contains a path separator",
    )
    .expect("invalid workspace package selector should report package-name validation");
}

#[test]
fn test_workspace_path_package_selector_surfaces_broken_member_metadata() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-package-selector-broken-member");
    let project_root = temp.path().join("workspace");
    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/broken"]
"#,
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace/packages/app/src/lib.ql",
        "pub fn helper() -> Int { return 1 }\n",
    );
    temp.write(
        "workspace/packages/app/tests/smoke.ql",
        "fn main() -> Int { return 0 }\n",
    );
    temp.write(
        "workspace/packages/broken/qlang.toml",
        r#"
[package]
version = "0.1.0"
"#,
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["test"])
        .arg(&project_root)
        .args(["--package", "app"]);
    let output = run_command_capture(
        &mut command,
        "`ql test --package` broken workspace member metadata",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-test-package-selector-broken-member",
        "broken workspace member package selector",
        &output,
        1,
    )
    .expect("workspace-path `ql test --package` should reject unresolved member metadata");
    expect_empty_stdout(
        "project-test-package-selector-broken-member",
        "broken workspace member package selector",
        &stdout,
    )
    .expect("broken workspace member package selector should not print stdout");
    expect_stderr_contains(
        "project-test-package-selector-broken-member",
        "broken workspace member package selector",
        &stderr.replace('\\', "/"),
        "error: `ql test` failed to inspect workspace member `packages/broken`: manifest",
    )
    .expect("broken workspace member package selector should surface the broken member");
    expect_stderr_contains(
        "project-test-package-selector-broken-member",
        "broken workspace member package selector",
        &stderr,
        "does not declare `[package].name`",
    )
    .expect("broken workspace member package selector should preserve package-name detail");
}

#[test]
fn test_workspace_path_package_selector_lists_selected_member_with_unbuildable_unselected_member() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-test-package-selector-unselected-source-root");
    let project_root = temp.path().join("workspace");
    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/tool"]
"#,
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace/packages/app/src/lib.ql",
        "pub fn helper() -> Int { return 1 }\n",
    );
    temp.write(
        "workspace/packages/app/tests/smoke.ql",
        "fn main() -> Int { return 0 }\n",
    );
    temp.write(
        "workspace/packages/tool/qlang.toml",
        r#"
[package]
name = "tool"
"#,
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["test"])
        .arg(&project_root)
        .args(["--package", "app", "--list"]);
    let output = run_command_capture(
        &mut command,
        "`ql test --package --list` skips unselected member target discovery",
    );
    let (stdout, stderr) = expect_success(
        "project-test-package-selector-unselected-source-root",
        "selected workspace package test listing with unbuildable unselected member",
        &output,
    )
    .expect(
        "workspace-path `ql test --package app --list` should not inspect unselected build targets",
    );
    expect_empty_stderr(
        "project-test-package-selector-unselected-source-root",
        "selected workspace package test listing with unbuildable unselected member",
        &stderr,
    )
    .expect("selected workspace package test listing should not print stderr");
    expect_stdout_contains_all(
        "project-test-package-selector-unselected-source-root",
        &stdout.replace('\\', "/"),
        &["packages/app/tests/smoke.ql", "test listing: 1 discovered"],
    )
    .expect("selected workspace package test listing should list only selected package tests");
}
