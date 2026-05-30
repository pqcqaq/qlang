mod support;

use support::{
    TempDir, expect_exit_code, expect_stderr_contains, expect_stderr_not_contains,
    expect_stdout_contains_all, expect_success, ql_command, run_command_capture, workspace_root,
};

#[test]
fn check_package_dir_reports_stale_dependency_interface() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-stale-interface");
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
        "workspace/dep/src/lib.ql",
        r#"
package demo.dep

pub fn exported() -> Int {
    return 7
}
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

    std::thread::sleep(std::time::Duration::from_millis(1200));
    temp.write(
        "workspace/dep/src/lib.ql",
        r#"
package demo.dep

pub fn exported() -> Int {
    return 9
}
"#,
    );

    let mut command = ql_command(&workspace_root);
    command.args(["check"]).arg(&app_root);
    let output = run_command_capture(&mut command, "`ql check` stale dependency interface");
    let (_stdout, stderr) = expect_exit_code(
        "project-check-stale-interface",
        "package-aware ql check with stale dependency interface",
        &output,
        1,
    )
    .expect("stale dependency interface should fail package-aware ql check");
    expect_stderr_contains(
        "project-check-stale-interface",
        "package-aware ql check with stale dependency interface",
        &stderr,
        "referenced package `dep` has stale interface artifact",
    )
    .expect("stale dependency interface should surface a clear error");
    expect_stderr_contains(
        "project-check-stale-interface",
        "package-aware ql check with stale dependency interface",
        &stderr,
        "reason: source newer than artifact:",
    )
    .expect("stale dependency interface should report why the artifact is stale");
    expect_stderr_contains(
        "project-check-stale-interface",
        "package-aware ql check with stale dependency interface",
        &stderr,
        "--sync-interfaces",
    )
    .expect("stale dependency interface diagnostic should suggest sync");
    let normalized_stderr = stderr.replace('\\', "/");
    let error_line = format!(
        "error: `ql check` referenced package `dep` has stale interface artifact `{}`",
        dep_root
            .join("dep.qi")
            .display()
            .to_string()
            .replace('\\', "/")
    );
    let old_error_line = format!(
        "error: referenced package `dep` has stale interface artifact `{}`",
        dep_root
            .join("dep.qi")
            .display()
            .to_string()
            .replace('\\', "/")
    );
    let reason_line = format!(
        "reason: source newer than artifact: {}",
        dep_root
            .join("src")
            .join("lib.ql")
            .display()
            .to_string()
            .replace('\\', "/")
    );
    let failing_manifest_note = format!(
        "note: failing referenced package manifest: {}",
        dep_root
            .join("qlang.toml")
            .display()
            .to_string()
            .replace('\\', "/")
    );
    let owner_note = format!(
        "note: while checking referenced package `../dep` from `{}`",
        app_root
            .join("qlang.toml")
            .display()
            .to_string()
            .replace('\\', "/")
    );
    let rerun_hint = format!(
        "hint: rerun `ql check --sync-interfaces {}` or regenerate `dep` with `ql project emit-interface {}`",
        app_root
            .join("qlang.toml")
            .display()
            .to_string()
            .replace('\\', "/"),
        dep_root
            .join("qlang.toml")
            .display()
            .to_string()
            .replace('\\', "/")
    );
    let error_index = normalized_stderr
        .find(&error_line)
        .expect("stale dependency interface should report the error line");
    expect_stderr_not_contains(
        "project-check-stale-interface",
        "package-aware ql check with stale dependency interface",
        &normalized_stderr,
        &old_error_line,
    )
    .expect("stale dependency interface should not fall back to the unlabeled artifact error");
    let reason_index = normalized_stderr
        .find(&reason_line)
        .expect("stale dependency interface should report the stale reason");
    let failing_manifest_index = normalized_stderr
        .find(&failing_manifest_note)
        .expect("stale dependency interface should point to the referenced manifest");
    let owner_note_index = normalized_stderr
        .find(&owner_note)
        .expect("stale dependency interface should point back to the owner manifest");
    let rerun_hint_index = normalized_stderr
        .find(&rerun_hint)
        .expect("stale dependency interface should include the repair hint");
    assert!(
        error_index < reason_index
            && reason_index < failing_manifest_index
            && failing_manifest_index < owner_note_index
            && owner_note_index < rerun_hint_index,
        "expected stale dependency interface diagnostic order error -> reason -> manifests -> hint, got:\n{stderr}"
    );
}

#[test]
fn check_package_dir_syncs_missing_dependency_interfaces() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-sync-interfaces");
    let dep_root = temp.path().join("workspace").join("dep");
    let app_root = temp.path().join("workspace").join("app");
    let source_path = app_root.join("src").join("lib.ql");
    let interface_path = dep_root.join("dep.qi");
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
        "workspace/dep/src/lib.ql",
        r#"
package demo.dep

pub fn exported() -> Int {
    return 7
}
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
    command.args(["check", "--sync-interfaces"]).arg(&app_root);
    let output = run_command_capture(&mut command, "`ql check --sync-interfaces` package dir");
    let (stdout, stderr) = expect_success(
        "project-check-sync-interfaces",
        "package-aware ql check with synced dependency interfaces",
        &output,
    )
    .expect("syncing missing dependency interfaces should let package-aware ql check succeed");
    expect_stdout_contains_all(
        "project-check-sync-interfaces",
        &stdout,
        &[
            "wrote interface: ",
            "dep.qi",
            &format!("ok: {}", source_path.display()),
            "loaded interface: ",
        ],
    )
    .expect("syncing missing dependency interfaces should report emitted and loaded interfaces");
    assert!(
        interface_path.is_file(),
        "expected synced dependency interface at `{}`",
        interface_path.display()
    );
    assert!(
        stderr.trim().is_empty(),
        "expected package-aware ql check stderr to stay empty, got:\n{stderr}"
    );
}

#[test]
fn check_package_dir_syncs_stale_dependency_interfaces() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-sync-stale-interfaces");
    let dep_root = temp.path().join("workspace").join("dep");
    let app_root = temp.path().join("workspace").join("app");
    let source_path = app_root.join("src").join("lib.ql");
    let interface_path = dep_root.join("dep.qi");
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
        "workspace/dep/src/lib.ql",
        r#"
package demo.dep

pub fn exported() -> Int {
    return 7
}
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

    std::thread::sleep(std::time::Duration::from_millis(1200));
    temp.write(
        "workspace/dep/src/lib.ql",
        r#"
package demo.dep

pub fn exported() -> Int {
    return 9
}
"#,
    );

    let mut command = ql_command(&workspace_root);
    command.args(["check", "--sync-interfaces"]).arg(&app_root);
    let output = run_command_capture(
        &mut command,
        "`ql check --sync-interfaces` stale package dir",
    );
    let (stdout, stderr) = expect_success(
        "project-check-sync-stale-interfaces",
        "package-aware ql check with synced stale dependency interfaces",
        &output,
    )
    .expect("syncing stale dependency interfaces should let package-aware ql check succeed");
    expect_stdout_contains_all(
        "project-check-sync-stale-interfaces",
        &stdout,
        &[
            "wrote interface: ",
            "dep.qi",
            &format!("ok: {}", source_path.display()),
            "loaded interface: ",
        ],
    )
    .expect("syncing stale dependency interfaces should report emitted and loaded interfaces");
    assert!(
        interface_path.is_file(),
        "expected synced dependency interface at `{}`",
        interface_path.display()
    );
    assert!(
        stderr.trim().is_empty(),
        "expected package-aware ql check stderr to stay empty, got:\n{stderr}"
    );
}

#[test]
fn check_source_file_syncs_missing_dependency_interfaces() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-sync-interfaces-source-file");
    let dep_root = temp.path().join("workspace").join("dep");
    let app_root = temp.path().join("workspace").join("app");
    let source_path = app_root.join("src").join("lib.ql");
    let interface_path = dep_root.join("dep.qi");
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
        "workspace/dep/src/lib.ql",
        r#"
package demo.dep

pub fn exported() -> Int {
    return 7
}
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
    command
        .args(["check", "--sync-interfaces"])
        .arg(&source_path);
    let output = run_command_capture(&mut command, "`ql check --sync-interfaces` source file");
    let (stdout, stderr) = expect_success(
        "project-check-sync-interfaces-source-file",
        "package-aware ql check with synced dependency interfaces from source file",
        &output,
    )
    .expect("syncing interfaces from a source file path should let package-aware ql check succeed");
    expect_stdout_contains_all(
        "project-check-sync-interfaces-source-file",
        &stdout,
        &[
            "wrote interface: ",
            "dep.qi",
            &format!("ok: {}", source_path.display()),
            "loaded interface: ",
        ],
    )
    .expect("source-file sync path should report emitted and loaded interfaces");
    assert!(
        interface_path.is_file(),
        "expected synced dependency interface at `{}`",
        interface_path.display()
    );
    assert!(
        stderr.trim().is_empty(),
        "expected package-aware ql check stderr to stay empty, got:\n{stderr}"
    );
}
