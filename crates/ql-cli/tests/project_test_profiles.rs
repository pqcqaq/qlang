mod support;

use std::path::PathBuf;

use ql_driver::{ToolchainOptions, discover_toolchain};
use support::{
    TempDir, executable_output_path, expect_empty_stderr, expect_file_exists,
    expect_stdout_contains_all, expect_success, ql_command, run_command_capture, workspace_root,
};

fn toolchain_available(context: &str) -> bool {
    let Ok(_toolchain) = discover_toolchain(&ToolchainOptions::default()) else {
        eprintln!(
            "skipping {context}: no clang-style compiler found via ql-driver toolchain discovery"
        );
        return false;
    };
    true
}

struct PackageProfileFixture {
    temp: TempDir,
    project_root: PathBuf,
    debug_smoke_output: PathBuf,
    release_smoke_output: PathBuf,
}

fn write_package_profile_fixture(prefix: &str, default_profile: &str) -> PackageProfileFixture {
    let temp = TempDir::new(prefix);
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create package source root for profile test");
    let manifest = format!(
        r#"
[package]
name = "app"

[profile]
default = "{default_profile}"
"#,
    );
    temp.write("app/qlang.toml", &manifest);
    temp.write("app/src/lib.ql", "pub fn helper() -> Int { return 1 }\n");
    temp.write("app/tests/smoke.ql", "fn main() -> Int { return 0 }\n");

    let debug_smoke_output =
        executable_output_path(&project_root.join("target/ql/debug/tests"), "smoke");
    let release_smoke_output =
        executable_output_path(&project_root.join("target/ql/release/tests"), "smoke");

    PackageProfileFixture {
        temp,
        project_root,
        debug_smoke_output,
        release_smoke_output,
    }
}

fn write_package_default_release_profile_fixture(prefix: &str) -> PackageProfileFixture {
    write_package_profile_fixture(prefix, "release")
}

fn write_package_default_debug_profile_fixture(prefix: &str) -> PackageProfileFixture {
    write_package_profile_fixture(prefix, "debug")
}

struct WorkspaceProfileFixture {
    temp: TempDir,
    project_root: PathBuf,
    smoke_path: PathBuf,
    debug_smoke_output: PathBuf,
    release_smoke_output: PathBuf,
    other_release_output: Option<PathBuf>,
}

fn write_workspace_profile_fixture(
    prefix: &str,
    default_profile: &str,
    include_other_test: bool,
) -> WorkspaceProfileFixture {
    let temp = TempDir::new(prefix);
    let project_root = temp.path().join("workspace");
    std::fs::create_dir_all(project_root.join("packages/app/src"))
        .expect("create workspace package source tree for workspace profile test");
    let manifest = format!(
        r#"
[workspace]
members = ["packages/app"]

[profile]
default = "{default_profile}"
"#,
    );
    temp.write("workspace/qlang.toml", &manifest);
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
    let smoke_path = temp.write(
        "workspace/packages/app/tests/smoke.ql",
        "fn main() -> Int { return 0 }\n",
    );
    let other_release_output = if include_other_test {
        temp.write(
            "workspace/packages/app/tests/other.ql",
            "fn main() -> Int { return 0 }\n",
        );
        Some(executable_output_path(
            &project_root.join("packages/app/target/ql/release/tests"),
            "other",
        ))
    } else {
        None
    };

    let debug_smoke_output = executable_output_path(
        &project_root.join("packages/app/target/ql/debug/tests"),
        "smoke",
    );
    let release_smoke_output = executable_output_path(
        &project_root.join("packages/app/target/ql/release/tests"),
        "smoke",
    );

    WorkspaceProfileFixture {
        temp,
        project_root,
        smoke_path,
        debug_smoke_output,
        release_smoke_output,
        other_release_output,
    }
}

fn write_workspace_default_release_profile_fixture(prefix: &str) -> WorkspaceProfileFixture {
    write_workspace_profile_fixture(prefix, "release", false)
}

fn write_workspace_default_debug_profile_fixture(prefix: &str) -> WorkspaceProfileFixture {
    write_workspace_profile_fixture(prefix, "debug", false)
}

fn write_workspace_member_file_profile_fixture(prefix: &str) -> WorkspaceProfileFixture {
    write_workspace_profile_fixture(prefix, "release", true)
}

fn expect_profile_stdout(case_name: &str, stdout: &str, target_line: &str) {
    expect_stdout_contains_all(
        case_name,
        &stdout.replace('\\', "/"),
        &[target_line, "test result: ok. 1 passed; 0 failed"],
    )
    .unwrap_or_else(|error| panic!("{error}"));
}

#[test]
fn test_package_path_uses_manifest_default_release_profile() {
    if !toolchain_available("`ql test` manifest profile test") {
        return;
    }

    let workspace_root = workspace_root();
    let fixture = write_package_default_release_profile_fixture("ql-project-test-manifest-profile");

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command.args(["test"]).arg(&fixture.project_root);
    let output = run_command_capture(&mut command, "`ql test` manifest default profile");
    let (stdout, stderr) = expect_success(
        "project-test-manifest-profile",
        "manifest default profile test",
        &output,
    )
    .expect("package-path `ql test` should honor the manifest default profile");
    expect_empty_stderr(
        "project-test-manifest-profile",
        "manifest default profile test",
        &stderr,
    )
    .expect("manifest default profile test should not print stderr");
    expect_profile_stdout(
        "project-test-manifest-profile",
        &stdout,
        "test tests/smoke.ql ... ok",
    );
    expect_file_exists(
        "project-test-manifest-profile",
        &fixture.release_smoke_output,
        "manifest default profile smoke executable",
        "manifest default profile test",
    )
    .expect("manifest default profile test should emit smoke test artifacts under release");
}

#[test]
fn test_package_path_profile_override_keeps_debug_profile() {
    if !toolchain_available("`ql test --profile debug` manifest profile override test") {
        return;
    }

    let workspace_root = workspace_root();
    let fixture = write_package_default_release_profile_fixture("ql-project-test-profile-override");

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["test"])
        .arg(&fixture.project_root)
        .args(["--profile", "debug"]);
    let output = run_command_capture(&mut command, "`ql test --profile debug` manifest override");
    let (stdout, stderr) = expect_success(
        "project-test-profile-override",
        "manifest profile override test",
        &output,
    )
    .expect("package-path `ql test --profile debug` should override manifest default profile");
    expect_empty_stderr(
        "project-test-profile-override",
        "manifest profile override test",
        &stderr,
    )
    .expect("manifest profile override test should not print stderr");
    expect_profile_stdout(
        "project-test-profile-override",
        &stdout,
        "test tests/smoke.ql ... ok",
    );
    expect_file_exists(
        "project-test-profile-override",
        &fixture.debug_smoke_output,
        "manifest profile override smoke executable",
        "manifest profile override test",
    )
    .expect("manifest profile override test should emit smoke test artifacts under debug");
    assert!(
        !fixture.release_smoke_output.exists(),
        "manifest profile override test should not emit release artifacts"
    );
}

#[test]
fn test_package_path_release_flag_overrides_manifest_default_profile() {
    if !toolchain_available("`ql test --release` manifest profile override test") {
        return;
    }

    let workspace_root = workspace_root();
    let fixture = write_package_default_debug_profile_fixture("ql-project-test-release-override");

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["test"])
        .arg(&fixture.project_root)
        .arg("--release");
    let output = run_command_capture(&mut command, "`ql test --release` manifest override");
    let (stdout, stderr) = expect_success(
        "project-test-release-override",
        "manifest release alias override test",
        &output,
    )
    .expect("package-path `ql test --release` should override manifest default profile");
    expect_empty_stderr(
        "project-test-release-override",
        "manifest release alias override test",
        &stderr,
    )
    .expect("manifest release alias override test should not print stderr");
    expect_profile_stdout(
        "project-test-release-override",
        &stdout,
        "test tests/smoke.ql ... ok",
    );
    expect_file_exists(
        "project-test-release-override",
        &fixture.release_smoke_output,
        "manifest release alias override smoke executable",
        "manifest release alias override test",
    )
    .expect("manifest release alias override test should emit smoke test artifacts under release");
    assert!(
        !fixture.debug_smoke_output.exists(),
        "manifest release alias override test should not emit debug artifacts"
    );
}

#[test]
fn test_workspace_path_uses_workspace_default_profile() {
    if !toolchain_available("`ql test` workspace profile test") {
        return;
    }

    let workspace_root = workspace_root();
    let fixture =
        write_workspace_default_release_profile_fixture("ql-project-test-workspace-profile");

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command.args(["test"]).arg(&fixture.project_root);
    let output = run_command_capture(&mut command, "`ql test` workspace default profile");
    let (stdout, stderr) = expect_success(
        "project-test-workspace-profile",
        "workspace default profile test",
        &output,
    )
    .expect("workspace-path `ql test` should honor the workspace default profile");
    expect_empty_stderr(
        "project-test-workspace-profile",
        "workspace default profile test",
        &stderr,
    )
    .expect("workspace default profile test should not print stderr");
    expect_profile_stdout(
        "project-test-workspace-profile",
        &stdout,
        "test packages/app/tests/smoke.ql ... ok",
    );
    expect_file_exists(
        "project-test-workspace-profile",
        &fixture.release_smoke_output,
        "workspace default profile smoke executable",
        "workspace default profile test",
    )
    .expect("workspace default profile test should emit smoke test artifacts under release");
}

#[test]
fn test_workspace_path_profile_override_keeps_debug_profile() {
    if !toolchain_available("`ql test --profile debug` workspace profile override test") {
        return;
    }

    let workspace_root = workspace_root();
    let fixture = write_workspace_default_release_profile_fixture(
        "ql-project-test-workspace-profile-override",
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["test"])
        .arg(&fixture.project_root)
        .args(["--profile", "debug"]);
    let output = run_command_capture(&mut command, "`ql test --profile debug` workspace override");
    let (stdout, stderr) = expect_success(
        "project-test-workspace-profile-override",
        "workspace profile override test",
        &output,
    )
    .expect("workspace-path `ql test --profile debug` should override workspace default profile");
    expect_empty_stderr(
        "project-test-workspace-profile-override",
        "workspace profile override test",
        &stderr,
    )
    .expect("workspace profile override test should not print stderr");
    expect_profile_stdout(
        "project-test-workspace-profile-override",
        &stdout,
        "test packages/app/tests/smoke.ql ... ok",
    );
    expect_file_exists(
        "project-test-workspace-profile-override",
        &fixture.debug_smoke_output,
        "workspace profile override smoke executable",
        "workspace profile override test",
    )
    .expect("workspace profile override test should emit smoke test artifacts under debug");
    assert!(
        !fixture.release_smoke_output.exists(),
        "workspace profile override test should not emit release artifacts"
    );
}

#[test]
fn test_workspace_path_release_flag_overrides_workspace_default_profile() {
    if !toolchain_available("`ql test --release` workspace profile override test") {
        return;
    }

    let workspace_root = workspace_root();
    let fixture =
        write_workspace_default_debug_profile_fixture("ql-project-test-workspace-release-override");

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["test"])
        .arg(&fixture.project_root)
        .arg("--release");
    let output = run_command_capture(&mut command, "`ql test --release` workspace override");
    let (stdout, stderr) = expect_success(
        "project-test-workspace-release-override",
        "workspace release alias override test",
        &output,
    )
    .expect("workspace-path `ql test --release` should override workspace default profile");
    expect_empty_stderr(
        "project-test-workspace-release-override",
        "workspace release alias override test",
        &stderr,
    )
    .expect("workspace release alias override test should not print stderr");
    expect_profile_stdout(
        "project-test-workspace-release-override",
        &stdout,
        "test packages/app/tests/smoke.ql ... ok",
    );
    expect_file_exists(
        "project-test-workspace-release-override",
        &fixture.release_smoke_output,
        "workspace release alias override smoke executable",
        "workspace release alias override test",
    )
    .expect("workspace release alias override test should emit smoke test artifacts under release");
    assert!(
        !fixture.debug_smoke_output.exists(),
        "workspace release alias override test should not emit debug artifacts"
    );
}

#[test]
fn test_workspace_member_file_uses_workspace_default_profile() {
    if !toolchain_available("`ql test` workspace member file profile test") {
        return;
    }

    let workspace_root = workspace_root();
    let fixture = write_workspace_member_file_profile_fixture(
        "ql-project-test-workspace-member-file-profile",
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command.args(["test"]).arg(&fixture.smoke_path);
    let output = run_command_capture(&mut command, "`ql test` workspace member file profile");
    let (stdout, stderr) = expect_success(
        "project-test-workspace-member-file-profile",
        "workspace member file profile test",
        &output,
    )
    .expect("workspace-member-file `ql test` should honor the outer workspace profile");
    expect_empty_stderr(
        "project-test-workspace-member-file-profile",
        "workspace member file profile test",
        &stderr,
    )
    .expect("workspace member file profile test should not print stderr");
    expect_profile_stdout(
        "project-test-workspace-member-file-profile",
        &stdout,
        "test packages/app/tests/smoke.ql ... ok",
    );
    assert!(
        !stdout
            .replace('\\', "/")
            .contains("packages/app/tests/other.ql"),
        "workspace member file profile test should not run unselected tests: {stdout}"
    );
    expect_file_exists(
        "project-test-workspace-member-file-profile",
        &fixture.release_smoke_output,
        "workspace member file smoke executable",
        "workspace member file profile test",
    )
    .expect(
        "workspace member file profile test should emit the selected smoke artifact under release",
    );
    let other_output = fixture
        .other_release_output
        .as_ref()
        .expect("workspace member profile fixture should include an unselected test");
    assert!(
        !other_output.exists(),
        "workspace member file profile test should not emit unselected test artifacts"
    );
    assert!(
        !fixture.debug_smoke_output.exists(),
        "workspace member file profile test should not silently fall back to the debug profile"
    );
}

#[test]
fn test_workspace_member_file_profile_override_keeps_debug_profile() {
    if !toolchain_available("`ql test --profile debug` workspace member file override test") {
        return;
    }

    let workspace_root = workspace_root();
    let fixture = write_workspace_default_release_profile_fixture(
        "ql-project-test-workspace-member-file-profile-override",
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["test"])
        .arg(&fixture.smoke_path)
        .args(["--profile", "debug"]);
    let output = run_command_capture(
        &mut command,
        "`ql test --profile debug` workspace member file override",
    );
    let (stdout, stderr) = expect_success(
        "project-test-workspace-member-file-profile-override",
        "workspace member file profile override test",
        &output,
    )
    .expect("workspace member file `ql test --profile debug` should override workspace default");
    expect_empty_stderr(
        "project-test-workspace-member-file-profile-override",
        "workspace member file profile override test",
        &stderr,
    )
    .expect("workspace member file profile override test should not print stderr");
    expect_profile_stdout(
        "project-test-workspace-member-file-profile-override",
        &stdout,
        "test packages/app/tests/smoke.ql ... ok",
    );
    expect_file_exists(
        "project-test-workspace-member-file-profile-override",
        &fixture.debug_smoke_output,
        "workspace member file profile override smoke executable",
        "workspace member file profile override test",
    )
    .expect("workspace member file profile override test should emit debug artifact");
    assert!(
        !fixture.release_smoke_output.exists(),
        "workspace member file profile override test should not emit release artifact"
    );
}
