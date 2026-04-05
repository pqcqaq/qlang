mod support;

use support::{
    TempDir, expect_exit_code, expect_stderr_contains, expect_stdout_contains_all, expect_success,
    ql_command, run_command_capture, workspace_root,
};

#[test]
fn check_package_dir_loads_referenced_interfaces() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check");
    let dep_root = temp.path().join("workspace").join("dep");
    let app_root = temp.path().join("workspace").join("app");
    let source_path = app_root.join("src").join("lib.ql");
    std::fs::create_dir_all(dep_root.join("src")).expect("create dependency source directory");
    std::fs::create_dir_all(app_root.join("src")).expect("create app source directory");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub fn exported() -> Int
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    temp.write(
        "workspace/app/src/lib.ql",
        r#"
package demo.app

pub fn main() -> Int {
    return 1
}
"#,
    );

    let mut command = ql_command(&workspace_root);
    command.args(["check"]).arg(&app_root);
    let output = run_command_capture(&mut command, "`ql check` package dir");
    let (stdout, stderr) =
        expect_success("project-check-success", "package-aware ql check", &output)
            .expect("package-aware ql check should succeed");
    expect_stdout_contains_all(
        "project-check-success",
        &stdout,
        &[
            &format!("ok: {}", source_path.display()),
            "loaded interface: ",
            "dep.qi",
        ],
    )
    .expect("package-aware ql check should report sources and loaded interfaces");
    assert!(
        stderr.trim().is_empty(),
        "expected package-aware ql check stderr to stay empty, got:\n{stderr}"
    );
}

#[test]
fn check_package_dir_reports_missing_dependency_interface() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-missing-interface");
    let dep_root = temp.path().join("workspace").join("dep");
    let app_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(dep_root.join("src")).expect("create dependency source directory");
    std::fs::create_dir_all(app_root.join("src")).expect("create app source directory");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    temp.write(
        "workspace/app/src/lib.ql",
        r#"
package demo.app

pub fn main() -> Int {
    return 1
}
"#,
    );

    let mut command = ql_command(&workspace_root);
    command.args(["check"]).arg(&app_root);
    let output = run_command_capture(&mut command, "`ql check` missing dependency interface");
    let (_stdout, stderr) = expect_exit_code(
        "project-check-missing-interface",
        "package-aware ql check with missing dependency interface",
        &output,
        1,
    )
    .expect("missing dependency interface should fail package-aware ql check");
    expect_stderr_contains(
        "project-check-missing-interface",
        "package-aware ql check with missing dependency interface",
        &stderr,
        "referenced package `dep` is missing interface artifact",
    )
    .expect("missing dependency interface should surface a clear error");
}
