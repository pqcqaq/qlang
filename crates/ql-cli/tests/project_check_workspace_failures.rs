mod support;

use support::{
    TempDir, expect_exit_code, expect_stderr_contains, expect_stderr_not_contains,
    expect_stdout_contains_all, ql_command, run_command_capture, workspace_root,
};

#[test]
fn check_workspace_root_dedupes_single_failing_member_summary() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-workspace-single-failure");
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
    command.args(["check"]).arg(&workspace_manifest);
    let output = run_command_capture(
        &mut command,
        "`ql check` workspace root with single failing member",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-check-workspace-single-failure",
        "workspace-root ql check with single failing member",
        &output,
        1,
    )
    .expect("workspace-root ql check with a single failing member should fail");
    let normalized_stdout = stdout.replace('\\', "/");
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stdout_contains_all(
        "project-check-workspace-single-failure",
        &normalized_stdout,
        &[&format!(
            "ok: {}",
            app_source.display().to_string().replace('\\', "/")
        )],
    )
    .expect("workspace-root ql check should still report healthy members before failing");
    expect_stderr_contains(
        "project-check-workspace-single-failure",
        "workspace-root ql check with single failing member",
        &normalized_stderr,
        &format!(
            "error: `ql check` invalid manifest `{}`",
            broken_root
                .join("qlang.toml")
                .display()
                .to_string()
                .replace('\\', "/")
        ),
    )
    .expect(
        "workspace-root ql check should preserve the command label for broken member manifests",
    );
    expect_stderr_not_contains(
        "project-check-workspace-single-failure",
        "workspace-root ql check with single failing member",
        &normalized_stderr,
        &format!(
            "error: invalid manifest `{}`",
            broken_root
                .join("qlang.toml")
                .display()
                .to_string()
                .replace('\\', "/")
        ),
    )
    .expect("workspace-root ql check should not fall back to the generic broken member error line");
    expect_stderr_contains(
        "project-check-workspace-single-failure",
        "workspace-root ql check with single failing member",
        &normalized_stderr,
        &format!(
            "note: failing package manifest: {}",
            broken_root
                .join("qlang.toml")
                .display()
                .to_string()
                .replace('\\', "/")
        ),
    )
    .expect("workspace-root ql check should point the broken member at the package manifest");
    expect_stderr_contains(
        "project-check-workspace-single-failure",
        "workspace-root ql check with single failing member",
        &normalized_stderr,
        &format!(
            "note: failing workspace member manifest: {}",
            broken_root
                .join("qlang.toml")
                .display()
                .to_string()
                .replace('\\', "/")
        ),
    )
    .expect("workspace-root ql check should point the broken member locally");
    let rerun_hint = format!(
        "hint: rerun `ql check {}` after fixing the package manifest",
        broken_root
            .join("qlang.toml")
            .display()
            .to_string()
            .replace('\\', "/")
    );
    expect_stderr_contains(
        "project-check-workspace-single-failure",
        "workspace-root ql check with single failing member",
        &normalized_stderr,
        &rerun_hint,
    )
    .expect(
        "workspace-root ql check should suggest rerunning the broken member after fixing the package manifest",
    );
    expect_stderr_contains(
        "project-check-workspace-single-failure",
        "workspace-root ql check with single failing member",
        &stderr,
        "`ql check` found 1 failing member(s)",
    )
    .expect("workspace-root ql check should summarize the single failing member");
    expect_stderr_not_contains(
        "project-check-workspace-single-failure",
        "workspace-root ql check with single failing member",
        &normalized_stderr,
        "note: first failing member manifest:",
    )
    .expect("single failing workspace members should not repeat the manifest in the final summary");
}

#[test]
fn check_workspace_root_preserves_non_package_member_rerun_hint() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-workspace-non-package-member");
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
    command.args(["check"]).arg(&workspace_manifest);
    let output = run_command_capture(
        &mut command,
        "`ql check` workspace root with non-package member",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-check-workspace-non-package-member",
        "workspace-root ql check with non-package member",
        &output,
        1,
    )
    .expect("workspace-root ql check with non-package member should fail");
    let normalized_stdout = stdout.replace('\\', "/");
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stdout_contains_all(
        "project-check-workspace-non-package-member",
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
    let error_line =
        format!("error: `ql check` manifest `{broken_manifest}` does not declare `[package].name`");
    let old_error_line =
        format!("error: manifest `{broken_manifest}` does not declare `[package].name`");
    let package_note = format!("note: failing package manifest: {broken_manifest}");
    let member_note = format!("note: failing workspace member manifest: {broken_manifest}");
    let rerun_hint =
        format!("hint: rerun `ql check {broken_manifest}` after fixing the package manifest");
    expect_stderr_contains(
        "project-check-workspace-non-package-member",
        "workspace-root ql check with non-package member",
        &normalized_stderr,
        &error_line,
    )
    .expect(
        "workspace-root ql check should preserve the direct command label for non-package members",
    );
    expect_stderr_not_contains(
        "project-check-workspace-non-package-member",
        "workspace-root ql check with non-package member",
        &normalized_stderr,
        &old_error_line,
    )
    .expect("workspace-root ql check should not fall back to the generic non-package error line");
    let error_line_index = normalized_stderr
        .find(&error_line)
        .expect("workspace-root ql check should include the non-package member error");
    let package_note_index = normalized_stderr
        .find(&package_note)
        .expect("workspace-root ql check should include the package manifest note");
    let member_note_index = normalized_stderr
        .find(&member_note)
        .expect("workspace-root ql check should include the local member note");
    let rerun_hint_index = normalized_stderr
        .find(&rerun_hint)
        .expect("workspace-root ql check should include the rerun hint");
    assert!(
        error_line_index < package_note_index
            && package_note_index < member_note_index
            && member_note_index < rerun_hint_index,
        "expected non-package member context before rerun hint, got:\n{stderr}"
    );
    expect_stderr_contains(
        "project-check-workspace-non-package-member",
        "workspace-root ql check with non-package member",
        &stderr,
        "`ql check` found 1 failing member(s)",
    )
    .expect("workspace-root ql check should summarize the single non-package member");
    expect_stderr_not_contains(
        "project-check-workspace-non-package-member",
        "workspace-root ql check with non-package member",
        &normalized_stderr,
        "note: first failing member manifest:",
    )
    .expect(
        "single non-package workspace members should not repeat the manifest in the final summary",
    );
}

#[test]
fn check_workspace_root_preserves_missing_member_package_name_rerun_hint() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-workspace-missing-member-package-name");
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
    command.args(["check"]).arg(&workspace_manifest);
    let output = run_command_capture(
        &mut command,
        "`ql check` workspace root with missing member package name",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-check-workspace-missing-member-package-name",
        "workspace-root ql check with missing member package name",
        &output,
        1,
    )
    .expect("workspace-root ql check with missing member package name should fail");
    let normalized_stdout = stdout.replace('\\', "/");
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stdout_contains_all(
        "project-check-workspace-missing-member-package-name",
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
    let error_line =
        format!("error: `ql check` manifest `{broken_manifest}` does not declare `[package].name`");
    let old_error_line =
        format!("error: manifest `{broken_manifest}` does not declare `[package].name`");
    let package_note = format!("note: failing package manifest: {broken_manifest}");
    let member_note = format!("note: failing workspace member manifest: {broken_manifest}");
    let rerun_hint =
        format!("hint: rerun `ql check {broken_manifest}` after fixing the package manifest");
    expect_stderr_contains(
        "project-check-workspace-missing-member-package-name",
        "workspace-root ql check with missing member package name",
        &normalized_stderr,
        &error_line,
    )
    .expect(
        "workspace-root ql check should preserve the direct command label for missing member package names",
    );
    expect_stderr_not_contains(
        "project-check-workspace-missing-member-package-name",
        "workspace-root ql check with missing member package name",
        &normalized_stderr,
        &old_error_line,
    )
    .expect(
        "workspace-root ql check should not fall back to the generic missing member package-name error line",
    );
    let error_line_index = normalized_stderr
        .find(&error_line)
        .expect("workspace-root ql check should include the missing member package-name error");
    let package_note_index = normalized_stderr
        .find(&package_note)
        .expect("workspace-root ql check should include the package manifest note");
    let member_note_index = normalized_stderr
        .find(&member_note)
        .expect("workspace-root ql check should include the local member note");
    let rerun_hint_index = normalized_stderr
        .find(&rerun_hint)
        .expect("workspace-root ql check should include the rerun hint");
    assert!(
        error_line_index < package_note_index
            && package_note_index < member_note_index
            && member_note_index < rerun_hint_index,
        "expected missing member package-name context before rerun hint, got:\n{stderr}"
    );
    expect_stderr_contains(
        "project-check-workspace-missing-member-package-name",
        "workspace-root ql check with missing member package name",
        &stderr,
        "`ql check` found 1 failing member(s)",
    )
    .expect("workspace-root ql check should summarize the single missing member package name");
    expect_stderr_not_contains(
        "project-check-workspace-missing-member-package-name",
        "workspace-root ql check with missing member package name",
        &normalized_stderr,
        "note: first failing member manifest:",
    )
    .expect(
        "single missing member package-name failures should not repeat the manifest in the final summary",
    );
}

#[test]
fn check_workspace_root_reports_all_failing_members() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-workspace-failures");
    let good_dep_root = temp.path().join("workspace").join("deps").join("good");
    let missing_dep_root = temp.path().join("workspace").join("deps").join("missing");
    let good_root = temp.path().join("workspace").join("packages").join("good");
    let missing_root = temp
        .path()
        .join("workspace")
        .join("packages")
        .join("missing");
    let broken_root = temp
        .path()
        .join("workspace")
        .join("packages")
        .join("broken");
    let good_source = good_root.join("src").join("lib.ql");
    let workspace_manifest = temp.path().join("workspace");
    std::fs::create_dir_all(good_dep_root.join("src"))
        .expect("create good dependency source directory");
    std::fs::create_dir_all(missing_dep_root.join("src"))
        .expect("create missing dependency source directory");
    std::fs::create_dir_all(good_root.join("src")).expect("create good package source directory");
    std::fs::create_dir_all(missing_root.join("src"))
        .expect("create missing package source directory");
    std::fs::create_dir_all(broken_root.join("src"))
        .expect("create broken package source directory");

    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/good", "packages/missing", "packages/broken"]
"#,
    );
    temp.write(
        "workspace/deps/good/qlang.toml",
        r#"
[package]
name = "good_dep"
"#,
    );
    temp.write(
        "workspace/deps/good/good_dep.qi",
        r#"
// qlang interface v1
// package: good_dep

// source: src/lib.ql
package demo.good_dep

pub fn exported() -> Int
"#,
    );
    temp.write(
        "workspace/deps/missing/qlang.toml",
        r#"
[package]
name = "missing_dep"
"#,
    );
    temp.write(
        "workspace/packages/good/qlang.toml",
        r#"
[package]
name = "good"

[references]
packages = ["../../deps/good"]
"#,
    );
    temp.write(
        "workspace/packages/good/src/lib.ql",
        r#"
package demo.good

pub fn main() -> Int {
    return 1
}
"#,
    );
    temp.write(
        "workspace/packages/missing/qlang.toml",
        r#"
[package]
name = "missing"

[references]
packages = ["../../deps/missing"]
"#,
    );
    temp.write(
        "workspace/packages/missing/src/lib.ql",
        r#"
package demo.missing

pub fn main() -> Int {
    return 2
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
    return 3
}
"#,
    );

    let mut command = ql_command(&workspace_root);
    command.args(["check"]).arg(&workspace_manifest);
    let output = run_command_capture(
        &mut command,
        "`ql check` workspace root with multiple failing members",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-check-workspace-failures",
        "workspace-root ql check with multiple failing members",
        &output,
        1,
    )
    .expect("workspace-root ql check with multiple failing members should fail");
    let normalized_stdout = stdout.replace('\\', "/");
    let normalized_good_source = good_source.display().to_string().replace('\\', "/");
    expect_stdout_contains_all(
        "project-check-workspace-failures",
        &normalized_stdout,
        &[
            &format!("ok: {normalized_good_source}"),
            "loaded interface: ",
            "good_dep.qi",
        ],
    )
    .expect("workspace-root ql check should still report successful members before the summary");
    expect_stderr_contains(
        "project-check-workspace-failures",
        "workspace-root ql check with multiple failing members",
        &stderr,
        "error: `ql check` referenced package `missing_dep` is missing interface artifact",
    )
    .expect("workspace-root ql check should preserve the ql check command label for missing dependency interfaces");
    expect_stderr_not_contains(
        "project-check-workspace-failures",
        "workspace-root ql check with multiple failing members",
        &stderr,
        "error: referenced package `missing_dep` is missing interface artifact",
    )
    .expect("workspace-root ql check should not fall back to the unlabeled artifact error");
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stderr_contains(
        "project-check-workspace-failures",
        "workspace-root ql check with multiple failing members",
        &normalized_stderr,
        &format!(
            "note: failing workspace member manifest: {}",
            missing_root
                .join("qlang.toml")
                .display()
                .to_string()
                .replace('\\', "/")
        ),
    )
    .expect("workspace-root ql check should point missing-reference failures at the member manifest immediately");
    expect_stderr_contains(
        "project-check-workspace-failures",
        "workspace-root ql check with multiple failing members",
        &normalized_stderr,
        "packages/broken/src/lib.ql",
    )
    .expect("workspace-root ql check should continue and surface later source diagnostics");
    expect_stderr_contains(
        "project-check-workspace-failures",
        "workspace-root ql check with multiple failing members",
        &normalized_stderr,
        &format!(
            "note: failing workspace member manifest: {}",
            broken_root
                .join("qlang.toml")
                .display()
                .to_string()
                .replace('\\', "/")
        ),
    )
    .expect(
        "workspace-root ql check should point source failures at the member manifest immediately",
    );
    expect_stderr_contains(
        "project-check-workspace-failures",
        "workspace-root ql check with multiple failing members",
        &stderr,
        "`ql check` found 2 failing member(s)",
    )
    .expect("workspace-root ql check should summarize all failing members");
    expect_stderr_contains(
        "project-check-workspace-failures",
        "workspace-root ql check with multiple failing members",
        &normalized_stderr,
        &format!(
            "note: first failing member manifest: {}",
            missing_root
                .join("qlang.toml")
                .display()
                .to_string()
                .replace('\\', "/")
        ),
    )
    .expect("workspace-root ql check should point to the first failing member manifest");
}
