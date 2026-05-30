mod support;

use support::{
    TempDir, expect_exit_code, expect_stderr_contains, expect_stderr_not_contains,
    expect_stdout_contains_all, ql_command, run_command_capture, workspace_root,
};

#[test]
fn check_workspace_root_sync_preserves_broken_member_manifest_label() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-workspace-sync-single-failure");
    let app_root = temp.path().join("workspace").join("packages").join("app");
    let broken_root = temp
        .path()
        .join("workspace")
        .join("packages")
        .join("broken");
    let app_source = app_root.join("src").join("lib.ql");
    let workspace_manifest = temp.path().join("workspace");
    std::fs::create_dir_all(app_root.join("src")).expect("create app source directory");
    std::fs::create_dir_all(&broken_root).expect("create broken member directory");

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
[package
name = "broken"
"#,
    );

    let mut command = ql_command(&workspace_root);
    command
        .args(["check", "--sync-interfaces"])
        .arg(&workspace_manifest);
    let output = run_command_capture(
        &mut command,
        "`ql check --sync-interfaces` workspace root with single failing member",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-check-workspace-sync-single-failure",
        "workspace-root ql check sync with single failing member",
        &output,
        1,
    )
    .expect("workspace-root ql check sync with a single failing member should fail");
    let normalized_stdout = stdout.replace('\\', "/");
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stdout_contains_all(
        "project-check-workspace-sync-single-failure",
        &normalized_stdout,
        &[&format!(
            "ok: {}",
            app_source.display().to_string().replace('\\', "/")
        )],
    )
    .expect("workspace-root ql check sync should still report healthy members before failing");
    let broken_manifest = broken_root
        .join("qlang.toml")
        .display()
        .to_string()
        .replace('\\', "/");
    let error_line =
        format!("error: `ql check --sync-interfaces` invalid manifest `{broken_manifest}`");
    let old_error_line = format!("error: invalid manifest `{broken_manifest}`");
    let package_note = format!("note: failing package manifest: {broken_manifest}");
    let member_note = format!("note: failing workspace member manifest: {broken_manifest}");
    let rerun_hint = format!(
        "hint: rerun `ql check --sync-interfaces {broken_manifest}` after fixing the package manifest"
    );
    expect_stderr_contains(
        "project-check-workspace-sync-single-failure",
        "workspace-root ql check sync with single failing member",
        &normalized_stderr,
        &error_line,
    )
    .expect("workspace-root ql check sync should preserve the command label for broken member manifests");
    expect_stderr_not_contains(
        "project-check-workspace-sync-single-failure",
        "workspace-root ql check sync with single failing member",
        &normalized_stderr,
        &old_error_line,
    )
    .expect(
        "workspace-root ql check sync should not fall back to the generic broken member error line",
    );
    expect_stderr_contains(
        "project-check-workspace-sync-single-failure",
        "workspace-root ql check sync with single failing member",
        &normalized_stderr,
        &package_note,
    )
    .expect("workspace-root ql check sync should point the broken member at the package manifest");
    expect_stderr_contains(
        "project-check-workspace-sync-single-failure",
        "workspace-root ql check sync with single failing member",
        &normalized_stderr,
        &member_note,
    )
    .expect("workspace-root ql check sync should point the broken member locally");
    expect_stderr_contains(
        "project-check-workspace-sync-single-failure",
        "workspace-root ql check sync with single failing member",
        &normalized_stderr,
        &rerun_hint,
    )
    .expect("workspace-root ql check sync should suggest rerunning the broken member after fixing the package manifest");
    expect_stderr_contains(
        "project-check-workspace-sync-single-failure",
        "workspace-root ql check sync with single failing member",
        &stderr,
        "`ql check --sync-interfaces` found 1 failing member(s)",
    )
    .expect("workspace-root ql check sync should summarize the single failing member");
    expect_stderr_not_contains(
        "project-check-workspace-sync-single-failure",
        "workspace-root ql check sync with single failing member",
        &normalized_stderr,
        "note: first failing member manifest:",
    )
    .expect(
        "single failing sync workspace members should not repeat the manifest in the final summary",
    );
}

#[test]
fn check_workspace_root_sync_preserves_non_package_member_rerun_hint() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-workspace-sync-non-package-member");
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
[workspace]
members = []
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
    command
        .args(["check", "--sync-interfaces"])
        .arg(&workspace_manifest);
    let output = run_command_capture(
        &mut command,
        "`ql check --sync-interfaces` workspace root with non-package member",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-check-workspace-sync-non-package-member",
        "workspace-root ql check sync with non-package member",
        &output,
        1,
    )
    .expect("workspace-root ql check sync with non-package member should fail");
    let normalized_stdout = stdout.replace('\\', "/");
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stdout_contains_all(
        "project-check-workspace-sync-non-package-member",
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
    .expect("workspace-root ql check sync should continue checking later valid members");
    let broken_manifest = broken_root
        .join("qlang.toml")
        .display()
        .to_string()
        .replace('\\', "/");
    let error_line = format!(
        "error: `ql check --sync-interfaces` manifest `{broken_manifest}` does not declare `[package].name`"
    );
    let old_error_line =
        format!("error: manifest `{broken_manifest}` does not declare `[package].name`");
    let package_note = format!("note: failing package manifest: {broken_manifest}");
    let member_note = format!("note: failing workspace member manifest: {broken_manifest}");
    let rerun_hint = format!(
        "hint: rerun `ql check --sync-interfaces {broken_manifest}` after fixing the package manifest"
    );
    expect_stderr_contains(
        "project-check-workspace-sync-non-package-member",
        "workspace-root ql check sync with non-package member",
        &normalized_stderr,
        &error_line,
    )
    .expect("workspace-root ql check sync should preserve the direct command label for non-package members");
    expect_stderr_not_contains(
        "project-check-workspace-sync-non-package-member",
        "workspace-root ql check sync with non-package member",
        &normalized_stderr,
        &old_error_line,
    )
    .expect(
        "workspace-root ql check sync should not fall back to the generic non-package error line",
    );
    let error_line_index = normalized_stderr
        .find(&error_line)
        .expect("workspace-root ql check sync should include the non-package member error");
    let package_note_index = normalized_stderr
        .find(&package_note)
        .expect("workspace-root ql check sync should include the package manifest note");
    let member_note_index = normalized_stderr
        .find(&member_note)
        .expect("workspace-root ql check sync should include the local member note");
    let rerun_hint_index = normalized_stderr
        .find(&rerun_hint)
        .expect("workspace-root ql check sync should include the rerun hint");
    assert!(
        error_line_index < package_note_index
            && package_note_index < member_note_index
            && member_note_index < rerun_hint_index,
        "expected sync non-package member context before rerun hint, got:\n{stderr}"
    );
    expect_stderr_contains(
        "project-check-workspace-sync-non-package-member",
        "workspace-root ql check sync with non-package member",
        &stderr,
        "`ql check --sync-interfaces` found 1 failing member(s)",
    )
    .expect("workspace-root ql check sync should summarize the single non-package member");
    expect_stderr_not_contains(
        "project-check-workspace-sync-non-package-member",
        "workspace-root ql check sync with non-package member",
        &normalized_stderr,
        "note: first failing member manifest:",
    )
    .expect("single sync non-package workspace members should not repeat the manifest in the final summary");
}

#[test]
fn check_workspace_root_sync_preserves_missing_member_package_name_rerun_hint() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-workspace-sync-missing-member-package-name");
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
version = "0.1.0"
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
    command
        .args(["check", "--sync-interfaces"])
        .arg(&workspace_manifest);
    let output = run_command_capture(
        &mut command,
        "`ql check --sync-interfaces` workspace root with missing member package name",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-check-workspace-sync-missing-member-package-name",
        "workspace-root ql check sync with missing member package name",
        &output,
        1,
    )
    .expect("workspace-root ql check sync with missing member package name should fail");
    let normalized_stdout = stdout.replace('\\', "/");
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stdout_contains_all(
        "project-check-workspace-sync-missing-member-package-name",
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
    .expect("workspace-root ql check sync should continue checking later valid members");
    let broken_manifest = broken_root
        .join("qlang.toml")
        .display()
        .to_string()
        .replace('\\', "/");
    let error_line = format!(
        "error: `ql check --sync-interfaces` manifest `{broken_manifest}` does not declare `[package].name`"
    );
    let old_error_line =
        format!("error: manifest `{broken_manifest}` does not declare `[package].name`");
    let package_note = format!("note: failing package manifest: {broken_manifest}");
    let member_note = format!("note: failing workspace member manifest: {broken_manifest}");
    let rerun_hint = format!(
        "hint: rerun `ql check --sync-interfaces {broken_manifest}` after fixing the package manifest"
    );
    expect_stderr_contains(
        "project-check-workspace-sync-missing-member-package-name",
        "workspace-root ql check sync with missing member package name",
        &normalized_stderr,
        &error_line,
    )
    .expect(
        "workspace-root ql check sync should preserve the direct command label for missing member package names",
    );
    expect_stderr_not_contains(
        "project-check-workspace-sync-missing-member-package-name",
        "workspace-root ql check sync with missing member package name",
        &normalized_stderr,
        &old_error_line,
    )
    .expect(
        "workspace-root ql check sync should not fall back to the generic missing member package-name error line",
    );
    let error_line_index = normalized_stderr.find(&error_line).expect(
        "workspace-root ql check sync should include the missing member package-name error",
    );
    let package_note_index = normalized_stderr
        .find(&package_note)
        .expect("workspace-root ql check sync should include the package manifest note");
    let member_note_index = normalized_stderr
        .find(&member_note)
        .expect("workspace-root ql check sync should include the local member note");
    let rerun_hint_index = normalized_stderr
        .find(&rerun_hint)
        .expect("workspace-root ql check sync should include the rerun hint");
    assert!(
        error_line_index < package_note_index
            && package_note_index < member_note_index
            && member_note_index < rerun_hint_index,
        "expected sync missing member package-name context before rerun hint, got:\n{stderr}"
    );
    expect_stderr_contains(
        "project-check-workspace-sync-missing-member-package-name",
        "workspace-root ql check sync with missing member package name",
        &stderr,
        "`ql check --sync-interfaces` found 1 failing member(s)",
    )
    .expect("workspace-root ql check sync should summarize the single missing member package name");
    expect_stderr_not_contains(
        "project-check-workspace-sync-missing-member-package-name",
        "workspace-root ql check sync with missing member package name",
        &normalized_stderr,
        "note: first failing member manifest:",
    )
    .expect(
        "single sync missing member package-name failures should not repeat the manifest in the final summary",
    );
}
