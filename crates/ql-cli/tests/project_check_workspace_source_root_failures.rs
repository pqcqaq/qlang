mod support;

use support::{
    TempDir, expect_exit_code, expect_stderr_contains, expect_stderr_not_contains,
    expect_stdout_contains_all, ql_command, run_command_capture, workspace_root,
};

#[test]
fn check_workspace_root_preserves_missing_source_root_rerun_hint() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-workspace-missing-source-root");
    let app_root = temp.path().join("workspace").join("packages").join("app");
    let broken_root = temp
        .path()
        .join("workspace")
        .join("packages")
        .join("broken");
    let tool_root = temp.path().join("workspace").join("packages").join("tool");
    let app_source = app_root.join("src").join("lib.ql");
    let tool_source = tool_root.join("src").join("lib.ql");
    let workspace_manifest = temp.path().join("workspace");
    std::fs::create_dir_all(app_root.join("src")).expect("create app source directory");
    std::fs::create_dir_all(&broken_root).expect("create broken member directory");
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
    return 2
}
"#,
    );

    let mut command = ql_command(&workspace_root);
    command.args(["check"]).arg(&workspace_manifest);
    let output = run_command_capture(
        &mut command,
        "`ql check` workspace root with missing source root member",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-check-workspace-missing-source-root",
        "workspace-root ql check with missing source root member",
        &output,
        1,
    )
    .expect("workspace-root ql check with missing source root member should fail");
    let normalized_stdout = stdout.replace('\\', "/");
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stdout_contains_all(
        "project-check-workspace-missing-source-root",
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
    let broken_source_root = broken_root
        .join("src")
        .display()
        .to_string()
        .replace('\\', "/");
    let error_line =
        format!("error: `ql check` package source directory `{broken_source_root}` does not exist");
    let old_error_line =
        format!("error: package source directory `{broken_source_root}` does not exist");
    let package_note = format!("note: failing package manifest: {broken_manifest}");
    let member_note = format!("note: failing workspace member manifest: {broken_manifest}");
    let source_root_note = format!("note: failing package source root: {broken_source_root}");
    let rerun_hint =
        format!("hint: rerun `ql check {broken_manifest}` after fixing the package source root");
    expect_stderr_contains(
        "project-check-workspace-missing-source-root",
        "workspace-root ql check with missing source root member",
        &normalized_stderr,
        &error_line,
    )
    .expect("workspace-root ql check should preserve the command label for missing source roots");
    expect_stderr_not_contains(
        "project-check-workspace-missing-source-root",
        "workspace-root ql check with missing source root member",
        &normalized_stderr,
        &old_error_line,
    )
    .expect(
        "workspace-root ql check should not fall back to the generic missing source-root error",
    );
    expect_stderr_contains(
        "project-check-workspace-missing-source-root",
        "workspace-root ql check with missing source root member",
        &normalized_stderr,
        &package_note,
    )
    .expect("workspace-root ql check should point to the failing package manifest");
    expect_stderr_contains(
        "project-check-workspace-missing-source-root",
        "workspace-root ql check with missing source root member",
        &normalized_stderr,
        &member_note,
    )
    .expect("workspace-root ql check should keep the workspace member boundary visible");
    expect_stderr_contains(
        "project-check-workspace-missing-source-root",
        "workspace-root ql check with missing source root member",
        &normalized_stderr,
        &source_root_note,
    )
    .expect("workspace-root ql check should point to the missing source root");
    expect_stderr_contains(
        "project-check-workspace-missing-source-root",
        "workspace-root ql check with missing source root member",
        &normalized_stderr,
        &rerun_hint,
    )
    .expect("workspace-root ql check should suggest rerunning the failing member manifest");
    let package_note_index = normalized_stderr
        .find(&package_note)
        .expect("workspace-root ql check should include the package note");
    let member_note_index = normalized_stderr
        .find(&member_note)
        .expect("workspace-root ql check should include the member note");
    let source_root_note_index = normalized_stderr
        .find(&source_root_note)
        .expect("workspace-root ql check should include the source-root note");
    let rerun_hint_index = normalized_stderr
        .find(&rerun_hint)
        .expect("workspace-root ql check should include the rerun hint");
    assert!(
        package_note_index < member_note_index
            && member_note_index < source_root_note_index
            && source_root_note_index < rerun_hint_index,
        "expected workspace missing-source-root context before hint, got:\n{stderr}"
    );
    expect_stderr_contains(
        "project-check-workspace-missing-source-root",
        "workspace-root ql check with missing source root member",
        &stderr,
        "`ql check` found 1 failing member(s)",
    )
    .expect("workspace-root ql check should summarize the single missing source-root member");
    expect_stderr_not_contains(
        "project-check-workspace-missing-source-root",
        "workspace-root ql check with missing source root member",
        &normalized_stderr,
        "note: first failing member manifest:",
    )
    .expect("single workspace missing source-root members should not repeat the manifest in the final summary");
}

#[test]
fn check_workspace_root_preserves_empty_source_root_rerun_hint() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-workspace-empty-source-root");
    let app_root = temp.path().join("workspace").join("packages").join("app");
    let broken_root = temp
        .path()
        .join("workspace")
        .join("packages")
        .join("broken");
    let tool_root = temp.path().join("workspace").join("packages").join("tool");
    let app_source = app_root.join("src").join("lib.ql");
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
    return 2
}
"#,
    );

    let mut command = ql_command(&workspace_root);
    command.args(["check"]).arg(&workspace_manifest);
    let output = run_command_capture(
        &mut command,
        "`ql check` workspace root with empty source root member",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-check-workspace-empty-source-root",
        "workspace-root ql check with empty source root member",
        &output,
        1,
    )
    .expect("workspace-root ql check with empty source root member should fail");
    let normalized_stdout = stdout.replace('\\', "/");
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stdout_contains_all(
        "project-check-workspace-empty-source-root",
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
    let broken_source_root = broken_root
        .join("src")
        .display()
        .to_string()
        .replace('\\', "/");
    let error_line = format!("error: `ql check` no `.ql` files found under `{broken_source_root}`");
    let old_error_line = format!("error: no `.ql` files found under `{broken_source_root}`");
    let package_note = format!("note: failing package manifest: {broken_manifest}");
    let member_note = format!("note: failing workspace member manifest: {broken_manifest}");
    let source_root_note = format!("note: failing package source root: {broken_source_root}");
    let rerun_hint =
        format!("hint: rerun `ql check {broken_manifest}` after adding package source files");
    expect_stderr_contains(
        "project-check-workspace-empty-source-root",
        "workspace-root ql check with empty source root member",
        &normalized_stderr,
        &error_line,
    )
    .expect("workspace-root ql check should preserve the command label for empty source roots");
    expect_stderr_not_contains(
        "project-check-workspace-empty-source-root",
        "workspace-root ql check with empty source root member",
        &normalized_stderr,
        &old_error_line,
    )
    .expect("workspace-root ql check should not fall back to the generic empty source-root error");
    expect_stderr_contains(
        "project-check-workspace-empty-source-root",
        "workspace-root ql check with empty source root member",
        &normalized_stderr,
        &package_note,
    )
    .expect("workspace-root ql check should point to the failing package manifest");
    expect_stderr_contains(
        "project-check-workspace-empty-source-root",
        "workspace-root ql check with empty source root member",
        &normalized_stderr,
        &member_note,
    )
    .expect("workspace-root ql check should keep the workspace member boundary visible");
    expect_stderr_contains(
        "project-check-workspace-empty-source-root",
        "workspace-root ql check with empty source root member",
        &normalized_stderr,
        &source_root_note,
    )
    .expect("workspace-root ql check should point to the empty source root");
    expect_stderr_contains(
        "project-check-workspace-empty-source-root",
        "workspace-root ql check with empty source root member",
        &normalized_stderr,
        &rerun_hint,
    )
    .expect("workspace-root ql check should suggest rerunning the failing member manifest");
    let package_note_index = normalized_stderr
        .find(&package_note)
        .expect("workspace-root ql check should include the package note");
    let member_note_index = normalized_stderr
        .find(&member_note)
        .expect("workspace-root ql check should include the member note");
    let source_root_note_index = normalized_stderr
        .find(&source_root_note)
        .expect("workspace-root ql check should include the source-root note");
    let rerun_hint_index = normalized_stderr
        .find(&rerun_hint)
        .expect("workspace-root ql check should include the rerun hint");
    assert!(
        package_note_index < member_note_index
            && member_note_index < source_root_note_index
            && source_root_note_index < rerun_hint_index,
        "expected workspace empty-source-root context before hint, got:\n{stderr}"
    );
    expect_stderr_contains(
        "project-check-workspace-empty-source-root",
        "workspace-root ql check with empty source root member",
        &stderr,
        "`ql check` found 1 failing member(s)",
    )
    .expect("workspace-root ql check should summarize the single empty source-root member");
    expect_stderr_not_contains(
        "project-check-workspace-empty-source-root",
        "workspace-root ql check with empty source root member",
        &normalized_stderr,
        "note: first failing member manifest:",
    )
    .expect("single workspace empty source-root members should not repeat the manifest in the final summary");
}
