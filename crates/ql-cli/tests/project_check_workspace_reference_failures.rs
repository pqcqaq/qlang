mod support;

use support::{
    TempDir, expect_exit_code, expect_stderr_contains, expect_stderr_not_contains,
    expect_stdout_contains_all, ql_command, run_command_capture, workspace_root,
};

#[test]
fn check_workspace_root_preserves_reference_failure_rerun_hint() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-workspace-reference-failure");
    let app_root = temp.path().join("workspace").join("packages").join("app");
    let broken_root = temp
        .path()
        .join("workspace")
        .join("packages")
        .join("broken");
    let tool_root = temp.path().join("workspace").join("packages").join("tool");
    let broken_ref_root = temp.path().join("workspace").join("broken_ref");
    let app_source = app_root.join("src").join("lib.ql");
    let tool_source = tool_root.join("src").join("lib.ql");
    let workspace_manifest = temp.path().join("workspace");
    std::fs::create_dir_all(app_root.join("src")).expect("create app source directory");
    std::fs::create_dir_all(broken_root.join("src")).expect("create broken source directory");
    std::fs::create_dir_all(tool_root.join("src")).expect("create tool source directory");
    std::fs::create_dir_all(&broken_ref_root).expect("create broken reference directory");

    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/broken", "packages/tool"]
"#,
    );
    temp.write(
        "workspace/broken_ref/qlang.toml",
        r#"
[package
name = "broken_ref"
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

[references]
packages = ["../../broken_ref"]
"#,
    );
    temp.write(
        "workspace/packages/broken/src/lib.ql",
        r#"
package demo.broken

pub fn main() -> Int {
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
        "`ql check` workspace root with reference failure",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-check-workspace-reference-failure",
        "workspace-root ql check with reference failure",
        &output,
        1,
    )
    .expect("workspace-root ql check with reference failure should fail");
    let normalized_stdout = stdout.replace('\\', "/");
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stdout_contains_all(
        "project-check-workspace-reference-failure",
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
    let broken_ref_manifest = broken_ref_root
        .join("qlang.toml")
        .display()
        .to_string()
        .replace('\\', "/");
    let reference_note = format!("note: failing reference manifest: {broken_ref_manifest}");
    let package_note = format!("note: failing package manifest: {broken_manifest}");
    let member_note = format!("note: failing workspace member manifest: {broken_manifest}");
    let rerun_hint = format!(
        "hint: rerun `ql check {broken_manifest}` after fixing the referenced package or reference manifest"
    );
    let error_line = "error: `ql check` failed to load referenced package `../../broken_ref`";
    let old_error_line = "error: failed to load referenced package `../../broken_ref`";
    expect_stderr_contains(
        "project-check-workspace-reference-failure",
        "workspace-root ql check with reference failure",
        &normalized_stderr,
        error_line,
    )
    .expect("workspace-root ql check should preserve the ql check command label");
    expect_stderr_not_contains(
        "project-check-workspace-reference-failure",
        "workspace-root ql check with reference failure",
        &normalized_stderr,
        old_error_line,
    )
    .expect("workspace-root ql check should not fall back to the unlabeled reference error");
    expect_stderr_contains(
        "project-check-workspace-reference-failure",
        "workspace-root ql check with reference failure",
        &normalized_stderr,
        &reference_note,
    )
    .expect("workspace-root ql check should point to the failing reference manifest");
    expect_stderr_contains(
        "project-check-workspace-reference-failure",
        "workspace-root ql check with reference failure",
        &normalized_stderr,
        &member_note,
    )
    .expect(
        "workspace-root ql check should point the reference failure back to the member manifest",
    );
    expect_stderr_contains(
        "project-check-workspace-reference-failure",
        "workspace-root ql check with reference failure",
        &normalized_stderr,
        &rerun_hint,
    )
    .expect("workspace-root ql check should suggest rerunning the failing member after fixing references");
    let reference_note_index = normalized_stderr
        .find(&reference_note)
        .expect("workspace-root ql check should include the reference note");
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
        reference_note_index < package_note_index
            && package_note_index < member_note_index
            && member_note_index < rerun_hint_index,
        "expected workspace reference failure context before member rerun hint, got:\n{stderr}"
    );
    expect_stderr_contains(
        "project-check-workspace-reference-failure",
        "workspace-root ql check with reference failure",
        &stderr,
        "`ql check` found 1 failing member(s)",
    )
    .expect("workspace-root ql check should summarize the single reference-failing member");
    expect_stderr_not_contains(
        "project-check-workspace-reference-failure",
        "workspace-root ql check with reference failure",
        &normalized_stderr,
        "note: first failing member manifest:",
    )
    .expect("single reference-failing workspace members should not repeat the manifest in the final summary");
}
