mod support;

use support::{
    TempDir, expect_empty_stderr, expect_exit_code, expect_stderr_contains,
    expect_stderr_not_contains, expect_stdout_contains_all, ql_command, run_command_capture,
    workspace_root,
};

#[test]
fn check_workspace_root_preserves_source_diagnostic_rerun_hint() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-workspace-source-diagnostics");
    let app_root = temp.path().join("workspace").join("packages").join("app");
    let broken_root = temp
        .path()
        .join("workspace")
        .join("packages")
        .join("broken");
    let tool_root = temp.path().join("workspace").join("packages").join("tool");
    let app_source = app_root.join("src").join("lib.ql");
    let broken_source = broken_root.join("src").join("lib.ql");
    let tool_source = tool_root.join("src").join("lib.ql");
    let workspace_manifest = temp.path().join("workspace");
    std::fs::create_dir_all(app_root.join("src")).expect("create app source directory");
    std::fs::create_dir_all(broken_root.join("src")).expect("create broken source directory");
    std::fs::create_dir_all(tool_root.join("src")).expect("create tool source directory");

    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/broken", "packages/tool"]
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
        r#"
package demo.app

pub fn main() -> Int {
    return 1
}
"#,
    );
    temp.write(
        "workspace/packages/broken/qlang.toml",
        r#"
[package]
name = "broken"
"#,
    );
    temp.write(
        "workspace/packages/broken/src/lib.ql",
        r#"
package demo.broken

pub fn main( -> Int {
    return 2
}
"#,
    );
    temp.write(
        "workspace/packages/tool/qlang.toml",
        r#"
[package]
name = "tool"
"#,
    );
    temp.write(
        "workspace/packages/tool/src/lib.ql",
        r#"
package demo.tool

pub fn main() -> Int {
    return 3
}
"#,
    );

    let mut command = ql_command(&workspace_root);
    command.args(["check"]).arg(&workspace_manifest);
    let output = run_command_capture(
        &mut command,
        "`ql check` workspace root with source diagnostics",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-check-workspace-source-diagnostics",
        "workspace-root ql check with source diagnostics",
        &output,
        1,
    )
    .expect("workspace-root ql check with source diagnostics should fail");
    let normalized_stdout = stdout.replace('\\', "/");
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stdout_contains_all(
        "project-check-workspace-source-diagnostics",
        &normalized_stdout,
        &[
            &format!(
                "ok: {}",
                app_source.display().to_string().replace('\\', "/")
            ),
            &format!(
                "ok: {}",
                tool_source.display().to_string().replace('\\', "/")
            ),
        ],
    )
    .expect("workspace-root ql check should continue checking later valid members");
    let broken_manifest = broken_root
        .join("qlang.toml")
        .display()
        .to_string()
        .replace('\\', "/");
    let package_note = format!("note: failing package manifest: {broken_manifest}");
    let member_note = format!("note: failing workspace member manifest: {broken_manifest}");
    let rerun_hint =
        format!("hint: rerun `ql check {broken_manifest}` after fixing the package sources");
    let broken_source_line = broken_source.display().to_string().replace('\\', "/");
    expect_stderr_contains(
        "project-check-workspace-source-diagnostics",
        "workspace-root ql check with source diagnostics",
        &normalized_stderr,
        &broken_source_line,
    )
    .expect("workspace-root ql check should surface the broken source path");
    expect_stderr_contains(
        "project-check-workspace-source-diagnostics",
        "workspace-root ql check with source diagnostics",
        &normalized_stderr,
        &member_note,
    )
    .expect("workspace-root ql check should point source diagnostics at the member manifest");
    expect_stderr_contains(
        "project-check-workspace-source-diagnostics",
        "workspace-root ql check with source diagnostics",
        &normalized_stderr,
        &rerun_hint,
    )
    .expect(
        "workspace-root ql check should suggest rerunning the broken member after fixing sources",
    );
    let broken_source_index = normalized_stderr
        .find(&broken_source_line)
        .expect("workspace-root ql check should include the broken source path");
    let package_note_index = normalized_stderr
        .find(&package_note)
        .expect("workspace-root ql check should include the package manifest note");
    let member_note_index = normalized_stderr
        .find(&member_note)
        .expect("workspace-root ql check should include the member note");
    let rerun_hint_index = normalized_stderr
        .find(&rerun_hint)
        .expect("workspace-root ql check should include the rerun hint");
    assert!(
        broken_source_index < package_note_index
            && package_note_index < member_note_index
            && member_note_index < rerun_hint_index,
        "expected workspace source diagnostics before member rerun hint, got:\n{stderr}"
    );
    expect_stderr_contains(
        "project-check-workspace-source-diagnostics",
        "workspace-root ql check with source diagnostics",
        &stderr,
        "`ql check` found 1 failing member(s)",
    )
    .expect("workspace-root ql check should summarize the single source-diagnostic member");
    expect_stderr_not_contains(
        "project-check-workspace-source-diagnostics",
        "workspace-root ql check with source diagnostics",
        &normalized_stderr,
        "note: first failing member manifest:",
    )
    .expect("single source-diagnostic workspace members should not repeat the manifest in the final summary");
}

#[test]
fn check_workspace_root_supports_json_source_diagnostics() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-workspace-json-source-diagnostics");
    let dep_root = temp.path().join("workspace").join("dep");
    let app_root = temp.path().join("workspace").join("packages").join("app");
    let broken_root = temp
        .path()
        .join("workspace")
        .join("packages")
        .join("broken");
    let tool_root = temp.path().join("workspace").join("packages").join("tool");
    let app_source = app_root.join("src").join("lib.ql");
    let broken_source = broken_root.join("src").join("lib.ql");
    let tool_source = tool_root.join("src").join("lib.ql");
    let workspace_manifest = temp.path().join("workspace");
    std::fs::create_dir_all(dep_root.join("src")).expect("create dependency source directory");
    std::fs::create_dir_all(app_root.join("src")).expect("create app source directory");
    std::fs::create_dir_all(broken_root.join("src")).expect("create broken source directory");
    std::fs::create_dir_all(tool_root.join("src")).expect("create tool source directory");

    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/broken", "packages/tool"]
"#,
    );
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
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../../dep"]
"#,
    );
    let broken_manifest = temp.write(
        "workspace/packages/broken/qlang.toml",
        r#"
[package]
name = "broken"
"#,
    );
    temp.write(
        "workspace/packages/tool/qlang.toml",
        r#"
[package]
name = "tool"
"#,
    );
    temp.write(
        "workspace/packages/app/src/lib.ql",
        r#"
package demo.app

pub fn main() -> Int {
    return 1
}
"#,
    );
    temp.write(
        "workspace/packages/broken/src/lib.ql",
        r#"
package demo.broken

pub fn main() -> Int {
    return "oops"
}
"#,
    );
    temp.write(
        "workspace/packages/tool/src/lib.ql",
        r#"
package demo.tool

pub fn main() -> Int {
    return 3
}
"#,
    );

    let mut command = ql_command(&workspace_root);
    command.args(["check", "--json"]).arg(&workspace_manifest);
    let output = run_command_capture(
        &mut command,
        "`ql check --json` workspace root with source diagnostics",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-check-workspace-json-source-diagnostics",
        "workspace-root ql check json with source diagnostics",
        &output,
        1,
    )
    .expect("workspace-root ql check json with source diagnostics should fail");
    expect_empty_stderr(
        "project-check-workspace-json-source-diagnostics",
        "workspace-root ql check json with source diagnostics",
        &stderr,
    )
    .expect("workspace-root ql check json with source diagnostics should keep stderr empty");

    let normalized_stdout = stdout.replace('\\', "/");
    expect_stdout_contains_all(
        "project-check-workspace-json-source-diagnostics",
        &normalized_stdout,
        &[
            "\"schema\": \"ql.check.v1\"",
            "\"scope\": \"workspace\"",
            "\"status\": \"diagnostics\"",
            &format!(
                "\"project_manifest_path\": \"{}\"",
                workspace_manifest
                    .join("qlang.toml")
                    .display()
                    .to_string()
                    .replace('\\', "/")
            ),
            &format!(
                "\"{}\"",
                app_source.display().to_string().replace('\\', "/")
            ),
            &format!(
                "\"{}\"",
                tool_source.display().to_string().replace('\\', "/")
            ),
            &format!(
                "\"{}\"",
                broken_source.display().to_string().replace('\\', "/")
            ),
            &format!(
                "\"{}\"",
                broken_manifest.display().to_string().replace('\\', "/")
            ),
            "\"message\": \"return value has type mismatch: expected `Int`, found `String`\"",
        ],
    )
    .expect("workspace-root ql check json should report healthy files and structured diagnostics");
}
