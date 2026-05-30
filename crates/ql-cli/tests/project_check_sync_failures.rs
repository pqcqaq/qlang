mod support;

use support::{
    TempDir, expect_exit_code, expect_stderr_contains, expect_stderr_not_contains,
    expect_stdout_contains_all, ql_command, run_command_capture, workspace_root,
};

#[test]
fn check_package_dir_sync_interfaces_reports_invalid_referenced_manifest() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-sync-invalid-reference-manifest");
    let app_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(app_root.join("src")).expect("create app source directory");
    std::fs::create_dir_all(temp.path().join("workspace").join("broken_ref"))
        .expect("create broken reference directory");

    temp.write(
        "workspace/broken_ref/qlang.toml",
        r#"
[package
name = "broken_ref"
"#,
    );
    let app_manifest = temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../broken_ref"]
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
    let output = run_command_capture(
        &mut command,
        "`ql check --sync-interfaces` invalid referenced manifest",
    );
    let (_stdout, stderr) = expect_exit_code(
        "project-check-sync-invalid-reference-manifest",
        "package-aware ql check sync with invalid referenced manifest",
        &output,
        1,
    )
    .expect("sync path should fail on invalid referenced manifest");
    let error_line =
        "error: `ql check --sync-interfaces` failed to load referenced package `../broken_ref`";
    let old_error_line = "error: failed to load referenced package `../broken_ref`";
    expect_stderr_contains(
        "project-check-sync-invalid-reference-manifest",
        "package-aware ql check sync with invalid referenced manifest",
        &stderr,
        error_line,
    )
    .expect("sync path should preserve the ql check sync command label");
    expect_stderr_not_contains(
        "project-check-sync-invalid-reference-manifest",
        "package-aware ql check sync with invalid referenced manifest",
        &stderr,
        old_error_line,
    )
    .expect("sync path should not fall back to the unlabeled reference error");
    expect_stderr_contains(
        "project-check-sync-invalid-reference-manifest",
        "package-aware ql check sync with invalid referenced manifest",
        &stderr,
        "invalid manifest `",
    )
    .expect("sync path should surface the manifest parse detail");
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stderr_contains(
        "project-check-sync-invalid-reference-manifest",
        "package-aware ql check sync with invalid referenced manifest",
        &normalized_stderr,
        &format!(
            "note: failing reference manifest: {}",
            temp.path()
                .join("workspace")
                .join("broken_ref")
                .join("qlang.toml")
                .display()
                .to_string()
                .replace('\\', "/")
        ),
    )
    .expect("sync path should point to the broken manifest path");
    expect_stderr_contains(
        "project-check-sync-invalid-reference-manifest",
        "package-aware ql check sync with invalid referenced manifest",
        &normalized_stderr,
        &format!(
            "fix the reference in `{}`",
            app_manifest.display().to_string().replace('\\', "/")
        ),
    )
    .expect("sync path should hint at the owning manifest");
    let package_note = format!(
        "note: failing package manifest: {}",
        app_manifest.display().to_string().replace('\\', "/")
    );
    let rerun_hint = format!(
        "hint: rerun `ql check --sync-interfaces {}` after fixing the referenced package or reference manifest",
        app_manifest.display().to_string().replace('\\', "/")
    );
    expect_stderr_contains(
        "project-check-sync-invalid-reference-manifest",
        "package-aware ql check sync with invalid referenced manifest",
        &normalized_stderr,
        &package_note,
    )
    .expect("sync path should point to the failing package manifest");
    expect_stderr_contains(
        "project-check-sync-invalid-reference-manifest",
        "package-aware ql check sync with invalid referenced manifest",
        &normalized_stderr,
        &rerun_hint,
    )
    .expect("sync path should suggest rerunning the package manifest directly");
    let reference_note = format!(
        "note: failing reference manifest: {}",
        temp.path()
            .join("workspace")
            .join("broken_ref")
            .join("qlang.toml")
            .display()
            .to_string()
            .replace('\\', "/")
    );
    let reference_note_index = normalized_stderr
        .find(&reference_note)
        .expect("sync path should include the failing reference note");
    let package_note_index = normalized_stderr
        .find(&package_note)
        .expect("sync path should include the package note");
    let rerun_hint_index = normalized_stderr
        .rfind(&rerun_hint)
        .expect("sync path should include the direct rerun hint");
    assert!(
        reference_note_index < package_note_index && package_note_index < rerun_hint_index,
        "expected direct package sync reference context before direct rerun hint, got:\n{stderr}"
    );
    expect_stderr_not_contains(
        "project-check-sync-invalid-reference-manifest",
        "package-aware ql check sync with invalid referenced manifest",
        &normalized_stderr,
        "note: first failing reference manifest:",
    )
    .expect("single failing references on the sync path should not repeat the manifest in the final summary");
}

#[test]
fn check_package_dir_sync_interfaces_reports_all_failing_references() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-sync-multiple-reference-failures");
    let dep_root = temp.path().join("workspace").join("dep");
    let app_root = temp.path().join("workspace").join("app");
    let interface_path = dep_root.join("dep.qi");
    std::fs::create_dir_all(dep_root.join("src")).expect("create dependency source directory");
    std::fs::create_dir_all(app_root.join("src")).expect("create app source directory");
    std::fs::create_dir_all(temp.path().join("workspace").join("broken_ref"))
        .expect("create broken reference directory");
    std::fs::create_dir_all(temp.path().join("workspace").join("workspace_ref"))
        .expect("create workspace-only reference directory");

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
        "workspace/broken_ref/qlang.toml",
        r#"
[package
name = "broken_ref"
"#,
    );
    temp.write(
        "workspace/workspace_ref/qlang.toml",
        r#"
[workspace]
members = ["packages/demo"]
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep", "../broken_ref", "../workspace_ref"]
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
    let output = run_command_capture(
        &mut command,
        "`ql check --sync-interfaces` multiple reference failures",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-check-sync-multiple-reference-failures",
        "package-aware ql check sync with multiple failing references",
        &output,
        1,
    )
    .expect("multiple failing references should fail package-aware sync");
    expect_stdout_contains_all(
        "project-check-sync-multiple-reference-failures",
        &stdout,
        &["wrote interface: ", "dep.qi"],
    )
    .expect("sync path should still emit interfaces for healthy references");
    assert!(
        interface_path.is_file(),
        "expected synced dependency interface at `{}`",
        interface_path.display()
    );
    expect_stderr_contains(
        "project-check-sync-multiple-reference-failures",
        "package-aware ql check sync with multiple failing references",
        &stderr,
        "failed to load referenced package `../broken_ref`",
    )
    .expect("sync path should surface broken manifests");
    expect_stderr_contains(
        "project-check-sync-multiple-reference-failures",
        "package-aware ql check sync with multiple failing references",
        &stderr,
        "failed to load referenced package `../workspace_ref`",
    )
    .expect("sync path should continue and surface later invalid package references");
    expect_stderr_contains(
        "project-check-sync-multiple-reference-failures",
        "package-aware ql check sync with multiple failing references",
        &stderr,
        "`ql check --sync-interfaces` found 2 failing referenced package(s)",
    )
    .expect("sync path should summarize all failing references");
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stderr_contains(
        "project-check-sync-multiple-reference-failures",
        "package-aware ql check sync with multiple failing references",
        &normalized_stderr,
        &format!(
            "note: first failing reference manifest: {}",
            temp.path()
                .join("workspace")
                .join("broken_ref")
                .join("qlang.toml")
                .display()
                .to_string()
                .replace('\\', "/")
        ),
    )
    .expect("sync path should point to the first failing reference manifest");
}

#[test]
fn check_package_dir_sync_interfaces_emits_direct_dependency_before_transitive_failure() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-sync-transitive-reference-failures");
    let dep_root = temp.path().join("workspace").join("dep");
    let app_root = temp.path().join("workspace").join("app");
    let interface_path = dep_root.join("dep.qi");
    std::fs::create_dir_all(dep_root.join("src")).expect("create dependency source directory");
    std::fs::create_dir_all(app_root.join("src")).expect("create app source directory");
    std::fs::create_dir_all(temp.path().join("workspace").join("broken_ref"))
        .expect("create broken reference directory");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"

[references]
packages = ["../broken_ref"]
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
        "workspace/broken_ref/qlang.toml",
        r#"
[package
name = "broken_ref"
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
    let output = run_command_capture(
        &mut command,
        "`ql check --sync-interfaces` transitive reference failures",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-check-sync-transitive-reference-failures",
        "package-aware ql check sync with transitive reference failures",
        &output,
        1,
    )
    .expect("transitive reference failures should still fail package-aware sync");
    expect_stdout_contains_all(
        "project-check-sync-transitive-reference-failures",
        &stdout,
        &["wrote interface: ", "dep.qi"],
    )
    .expect("sync path should still emit the direct dependency interface");
    assert!(
        interface_path.is_file(),
        "expected synced dependency interface at `{}`",
        interface_path.display()
    );
    expect_stderr_contains(
        "project-check-sync-transitive-reference-failures",
        "package-aware ql check sync with transitive reference failures",
        &stderr,
        "failed to load referenced package `../broken_ref`",
    )
    .expect("sync path should continue into transitive broken references");
    expect_stderr_contains(
        "project-check-sync-transitive-reference-failures",
        "package-aware ql check sync with transitive reference failures",
        &stderr,
        "`ql check --sync-interfaces` found 1 failing referenced package(s)",
    )
    .expect("sync path should only summarize the remaining transitive failure");
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stderr_not_contains(
        "project-check-sync-transitive-reference-failures",
        "package-aware ql check sync with transitive reference failures",
        &normalized_stderr,
        "note: first failing reference manifest:",
    )
    .expect(
        "single remaining transitive failures should not repeat the manifest in the final summary",
    );
}

#[test]
fn check_package_dir_sync_interfaces_points_dependency_source_failures_at_owner_reference() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-sync-source-failure-context");
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
        "workspace/dep/src/a_broken.ql",
        r#"
package demo.dep

pub fn broken_first(value: MissingFirst) -> Int {
    return value
}
"#,
    );
    temp.write(
        "workspace/dep/src/z_broken.ql",
        r#"
package demo.dep

pub fn broken_second(value: MissingSecond) -> Int {
    return value
}
"#,
    );
    let app_manifest = temp.write(
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
    let output = run_command_capture(
        &mut command,
        "`ql check --sync-interfaces` dependency source failure context",
    );
    let (_stdout, stderr) = expect_exit_code(
        "project-check-sync-source-failure-context",
        "package-aware ql check sync with dependency source failures",
        &output,
        1,
    )
    .expect("dependency source failures should fail package-aware sync");
    expect_stderr_contains(
        "project-check-sync-source-failure-context",
        "package-aware ql check sync with dependency source failures",
        &stderr,
        "a_broken.ql",
    )
    .expect("sync path should surface the first failing dependency source file");
    expect_stderr_contains(
        "project-check-sync-source-failure-context",
        "package-aware ql check sync with dependency source failures",
        &stderr,
        "z_broken.ql",
    )
    .expect("sync path should continue surfacing later dependency source failures");
    expect_stderr_contains(
        "project-check-sync-source-failure-context",
        "package-aware ql check sync with dependency source failures",
        &stderr,
        "`ql check --sync-interfaces` found 2 failing source file(s)",
    )
    .expect("sync path should preserve package-level source failure aggregation");
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stderr_contains(
        "project-check-sync-source-failure-context",
        "package-aware ql check sync with dependency source failures",
        &normalized_stderr,
        &format!(
            "note: first failing source file: {}",
            dep_root
                .join("src")
                .join("a_broken.ql")
                .display()
                .to_string()
                .replace('\\', "/")
        ),
    )
    .expect("sync path should point to the first failing dependency source file");
    expect_stderr_contains(
        "project-check-sync-source-failure-context",
        "package-aware ql check sync with dependency source failures",
        &normalized_stderr,
        &format!(
            "note: while syncing referenced package `../dep` from `{}`",
            app_manifest.display().to_string().replace('\\', "/")
        ),
    )
    .expect("sync path should point the dependency source failure back to the owner reference");
    expect_stderr_contains(
        "project-check-sync-source-failure-context",
        "package-aware ql check sync with dependency source failures",
        &normalized_stderr,
        &format!(
            "note: failing package manifest: {}",
            dep_root
                .join("qlang.toml")
                .display()
                .to_string()
                .replace('\\', "/")
        ),
    )
    .expect("sync path should point to the failing dependency manifest");
    expect_stderr_contains(
        "project-check-sync-source-failure-context",
        "package-aware ql check sync with dependency source failures",
        &normalized_stderr,
        &format!(
            "hint: rerun `ql project emit-interface {}` after fixing the package sources",
            dep_root
                .join("qlang.toml")
                .display()
                .to_string()
                .replace('\\', "/"),
        ),
    )
    .expect("sync path should reuse the standard package rerun hint");
    expect_stderr_not_contains(
        "project-check-sync-source-failure-context",
        "package-aware ql check sync with dependency source failures",
        &normalized_stderr,
        &format!(
            "hint: repair `{}` or rerun `ql project emit-interface {}` directly",
            dep_root
                .join("qlang.toml")
                .display()
                .to_string()
                .replace('\\', "/"),
            dep_root
                .join("qlang.toml")
                .display()
                .to_string()
                .replace('\\', "/")
        ),
    )
    .expect("sync path should not print the old duplicate direct-rerun hint");
    let package_note = format!(
        "note: failing package manifest: {}",
        dep_root
            .join("qlang.toml")
            .display()
            .to_string()
            .replace('\\', "/")
    );
    let owner_note = format!(
        "note: while syncing referenced package `../dep` from `{}`",
        app_manifest.display().to_string().replace('\\', "/")
    );
    let rerun_hint = format!(
        "hint: rerun `ql project emit-interface {}` after fixing the package sources",
        dep_root
            .join("qlang.toml")
            .display()
            .to_string()
            .replace('\\', "/"),
    );
    let package_note_index = normalized_stderr
        .find(&package_note)
        .expect("sync source failure should include the failing package manifest note");
    let owner_note_index = normalized_stderr
        .find(&owner_note)
        .expect("sync source failure should include the owner reference note");
    let rerun_hint_index = normalized_stderr
        .find(&rerun_hint)
        .expect("sync source failure should include the rerun hint");
    assert!(
        package_note_index < owner_note_index && owner_note_index < rerun_hint_index,
        "expected sync source failure context before rerun hint, got:\n{stderr}"
    );
    expect_stderr_contains(
        "project-check-sync-source-failure-context",
        "package-aware ql check sync with dependency source failures",
        &stderr,
        "`ql check --sync-interfaces` found 1 failing referenced package(s)",
    )
    .expect("sync path should still summarize the failing referenced package");
}

#[test]
fn check_package_dir_sync_interfaces_points_dependency_output_failures_at_interface_target() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-sync-output-path-failure");
    let dep_root = temp.path().join("workspace").join("dep");
    let app_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(dep_root.join("src"))
        .expect("create dependency source directory for sync output-path failure test");
    std::fs::create_dir_all(app_root.join("src"))
        .expect("create app source directory for sync output-path failure test");

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
    return 1
}
"#,
    );
    let interface_path = dep_root.join("dep.qi");
    std::fs::create_dir_all(&interface_path)
        .expect("create blocking interface directory for dependency sync test");
    let app_manifest = temp.write(
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

    let dep_manifest = dep_root.join("qlang.toml");
    let dep_manifest_display = dep_manifest.to_string_lossy().replace('\\', "/");
    let interface_display = interface_path.to_string_lossy().replace('\\', "/");
    let app_manifest_display = app_manifest.to_string_lossy().replace('\\', "/");

    let mut command = ql_command(&workspace_root);
    command.args(["check", "--sync-interfaces"]).arg(&app_root);
    let output = run_command_capture(
        &mut command,
        "`ql check --sync-interfaces` dependency blocked output path",
    );
    let (_stdout, stderr) = expect_exit_code(
        "project-check-sync-output-path-failure",
        "package-aware ql check sync with dependency blocked output path",
        &output,
        1,
    )
    .expect("dependency output-path failures should fail package-aware sync");
    expect_stderr_contains(
        "project-check-sync-output-path-failure",
        "package-aware ql check sync with dependency blocked output path",
        &stderr,
        "failed to write interface",
    )
    .expect("sync path should surface the dependency write failure");
    let normalized_stderr = stderr.replace('\\', "/");
    let package_note = format!("note: failing package manifest: {dep_manifest_display}");
    let output_note = format!("note: failing interface output path: {interface_display}");
    let owner_note =
        format!("note: while syncing referenced package `../dep` from `{app_manifest_display}`");
    let rerun_hint = format!(
        "hint: rerun `ql project emit-interface {}` after fixing the interface output path",
        dep_manifest_display
    );
    let old_hint = format!(
        "hint: rerun `ql project emit-interface {}` after fixing the package interface error",
        dep_manifest_display
    );
    expect_stderr_contains(
        "project-check-sync-output-path-failure",
        "package-aware ql check sync with dependency blocked output path",
        &normalized_stderr,
        &package_note,
    )
    .expect("sync path should still point to the dependency manifest");
    expect_stderr_contains(
        "project-check-sync-output-path-failure",
        "package-aware ql check sync with dependency blocked output path",
        &normalized_stderr,
        &output_note,
    )
    .expect("sync path should point to the blocked dependency interface path");
    expect_stderr_contains(
        "project-check-sync-output-path-failure",
        "package-aware ql check sync with dependency blocked output path",
        &normalized_stderr,
        &owner_note,
    )
    .expect("sync path should still point back to the owner reference");
    expect_stderr_contains(
        "project-check-sync-output-path-failure",
        "package-aware ql check sync with dependency blocked output path",
        &normalized_stderr,
        &rerun_hint,
    )
    .expect("sync path should suggest fixing the dependency output path");
    expect_stderr_not_contains(
        "project-check-sync-output-path-failure",
        "package-aware ql check sync with dependency blocked output path",
        &normalized_stderr,
        &old_hint,
    )
    .expect("sync path should not reuse the source-failure rerun hint for output-path failures");
    let package_note_index = normalized_stderr
        .find(&package_note)
        .expect("sync output-path failure should include the dependency manifest note");
    let output_note_index = normalized_stderr
        .find(&output_note)
        .expect("sync output-path failure should include the blocked interface path note");
    let owner_note_index = normalized_stderr
        .find(&owner_note)
        .expect("sync output-path failure should include the owner reference note");
    let rerun_hint_index = normalized_stderr
        .find(&rerun_hint)
        .expect("sync output-path failure should include the rerun hint");
    assert!(
        package_note_index < output_note_index
            && output_note_index < owner_note_index
            && owner_note_index < rerun_hint_index,
        "expected sync output-path failure context before rerun hint, got:\n{stderr}"
    );
    expect_stderr_contains(
        "project-check-sync-output-path-failure",
        "package-aware ql check sync with dependency blocked output path",
        &stderr,
        "`ql check --sync-interfaces` found 1 failing referenced package(s)",
    )
    .expect("sync path should still summarize the failing referenced package");
    assert!(
        interface_path.is_dir(),
        "sync output-path failure test should preserve `{}` as a directory",
        interface_path.display()
    );
}

#[test]
fn check_package_dir_sync_interfaces_points_dependency_missing_source_root_at_source_root() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-check-sync-missing-dependency-source-root");
    let dep_root = temp.path().join("workspace").join("dep");
    let app_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(&dep_root)
        .expect("create dependency directory for sync missing source root test");
    std::fs::create_dir_all(app_root.join("src"))
        .expect("create app source directory for sync missing source root test");

    let dep_manifest = temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    let app_manifest = temp.write(
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

    let dep_manifest_display = dep_manifest.to_string_lossy().replace('\\', "/");
    let dep_source_root_display = dep_root.join("src").to_string_lossy().replace('\\', "/");
    let app_manifest_display = app_manifest.to_string_lossy().replace('\\', "/");

    let mut command = ql_command(&workspace_root);
    command.args(["check", "--sync-interfaces"]).arg(&app_root);
    let output = run_command_capture(
        &mut command,
        "`ql check --sync-interfaces` dependency missing source root",
    );
    let (_stdout, stderr) = expect_exit_code(
        "project-check-sync-missing-dependency-source-root",
        "package-aware ql check sync with dependency missing source root",
        &output,
        1,
    )
    .expect("dependency missing source root should fail package-aware sync");
    let normalized_stderr = stderr.replace('\\', "/");
    let error_line = format!(
        "error: `ql check --sync-interfaces` package source directory `{dep_source_root_display}` does not exist"
    );
    let package_note = format!("note: failing package manifest: {dep_manifest_display}");
    let source_root_note = format!("note: failing package source root: {dep_source_root_display}");
    let owner_note =
        format!("note: while syncing referenced package `../dep` from `{app_manifest_display}`");
    let rerun_hint = format!(
        "hint: rerun `ql project emit-interface {dep_manifest_display}` after fixing the package source root"
    );
    let old_hint = format!(
        "hint: rerun `ql project emit-interface {dep_manifest_display}` after fixing the package interface error"
    );
    expect_stderr_contains(
        "project-check-sync-missing-dependency-source-root",
        "package-aware ql check sync with dependency missing source root",
        &normalized_stderr,
        &error_line,
    )
    .expect("sync path should preserve the command label for dependency source-root failures");
    expect_stderr_contains(
        "project-check-sync-missing-dependency-source-root",
        "package-aware ql check sync with dependency missing source root",
        &normalized_stderr,
        &package_note,
    )
    .expect("sync path should point to the dependency manifest");
    expect_stderr_contains(
        "project-check-sync-missing-dependency-source-root",
        "package-aware ql check sync with dependency missing source root",
        &normalized_stderr,
        &source_root_note,
    )
    .expect("sync path should point to the dependency source root");
    expect_stderr_contains(
        "project-check-sync-missing-dependency-source-root",
        "package-aware ql check sync with dependency missing source root",
        &normalized_stderr,
        &owner_note,
    )
    .expect("sync path should still point back to the owner reference");
    expect_stderr_contains(
        "project-check-sync-missing-dependency-source-root",
        "package-aware ql check sync with dependency missing source root",
        &normalized_stderr,
        &rerun_hint,
    )
    .expect("sync path should suggest fixing the dependency source root");
    expect_stderr_not_contains(
        "project-check-sync-missing-dependency-source-root",
        "package-aware ql check sync with dependency missing source root",
        &normalized_stderr,
        &old_hint,
    )
    .expect("sync path should not fall back to the generic dependency interface hint");
    let package_note_index = normalized_stderr
        .find(&package_note)
        .expect("sync missing source root should include the dependency manifest note");
    let source_root_note_index = normalized_stderr
        .find(&source_root_note)
        .expect("sync missing source root should include the dependency source root note");
    let owner_note_index = normalized_stderr
        .find(&owner_note)
        .expect("sync missing source root should include the owner reference note");
    let rerun_hint_index = normalized_stderr
        .find(&rerun_hint)
        .expect("sync missing source root should include the rerun hint");
    assert!(
        package_note_index < source_root_note_index
            && source_root_note_index < owner_note_index
            && owner_note_index < rerun_hint_index,
        "expected sync missing source-root context before rerun hint, got:\n{stderr}"
    );
    expect_stderr_contains(
        "project-check-sync-missing-dependency-source-root",
        "package-aware ql check sync with dependency missing source root",
        &stderr,
        "`ql check --sync-interfaces` found 1 failing referenced package(s)",
    )
    .expect("sync path should still summarize the failing referenced package");
}
