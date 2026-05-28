mod support;

use std::path::{Path, PathBuf};

use ql_driver::{ToolchainOptions, discover_toolchain};
use serde_json::Value as JsonValue;
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

fn normalize_output_text(text: &str) -> String {
    text.replace("\r\n", "\n")
}

fn parse_json_output(case_name: &str, stdout: &str) -> JsonValue {
    serde_json::from_str(&normalize_output_text(stdout))
        .unwrap_or_else(|error| panic!("[{case_name}] parse json stdout: {error}\n{stdout}"))
}

struct PackageProfileFixture {
    temp: TempDir,
    project_root: PathBuf,
    smoke_path: PathBuf,
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
    let smoke_path = temp.write("app/tests/smoke.ql", "fn main() -> Int { return 0 }\n");

    let debug_smoke_output =
        executable_output_path(&project_root.join("target/ql/debug/tests"), "smoke");
    let release_smoke_output =
        executable_output_path(&project_root.join("target/ql/release/tests"), "smoke");

    PackageProfileFixture {
        temp,
        project_root,
        smoke_path,
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

fn expected_package_profile_test_json(
    request_path: &Path,
    requested_profile: &str,
    profile_overridden: bool,
    target_profile: &str,
) -> JsonValue {
    expected_profile_test_json(
        request_path,
        "tests/smoke.ql",
        requested_profile,
        profile_overridden,
        target_profile,
        false,
    )
}

fn expected_package_profile_test_listing_json(
    request_path: &Path,
    requested_profile: &str,
    profile_overridden: bool,
    target_profile: &str,
) -> JsonValue {
    expected_profile_test_json(
        request_path,
        "tests/smoke.ql",
        requested_profile,
        profile_overridden,
        target_profile,
        true,
    )
}

fn expected_profile_test_json(
    request_path: &Path,
    target_path: &str,
    requested_profile: &str,
    profile_overridden: bool,
    target_profile: &str,
    list_only: bool,
) -> JsonValue {
    serde_json::json!({
        "schema": "ql.test.v1",
        "path": request_path.display().to_string().replace('\\', "/"),
        "requested_profile": requested_profile,
        "profile_overridden": profile_overridden,
        "package_name": JsonValue::Null,
        "filter": JsonValue::Null,
        "list_only": list_only,
        "status": if list_only { "listed" } else { "ok" },
        "discovered_total": 1,
        "selected_total": 1,
        "targets": [
            {
                "path": target_path,
                "kind": "smoke",
                "profile": target_profile,
            }
        ],
        "passed": if list_only { 0 } else { 1 },
        "failed": 0,
        "failures": [],
    })
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

fn expected_workspace_profile_test_json(
    request_path: &Path,
    requested_profile: &str,
    profile_overridden: bool,
    target_profile: &str,
) -> JsonValue {
    expected_profile_test_json(
        request_path,
        "packages/app/tests/smoke.ql",
        requested_profile,
        profile_overridden,
        target_profile,
        false,
    )
}

fn expected_workspace_profile_test_listing_json(
    request_path: &Path,
    requested_profile: &str,
    profile_overridden: bool,
    target_profile: &str,
) -> JsonValue {
    expected_profile_test_json(
        request_path,
        "packages/app/tests/smoke.ql",
        requested_profile,
        profile_overridden,
        target_profile,
        true,
    )
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

fn assert_profile_json(case_name: &str, stdout: &str, expected: JsonValue, failure_message: &str) {
    let actual = parse_json_output(case_name, stdout);
    assert_eq!(actual, expected, "{failure_message}");
}

fn assert_package_profile_artifacts_absent(fixture: &PackageProfileFixture, context: &str) {
    assert!(
        !fixture.debug_smoke_output.exists() && !fixture.release_smoke_output.exists(),
        "{context} should not emit test artifacts"
    );
}

fn assert_workspace_profile_artifacts_absent(fixture: &WorkspaceProfileFixture, context: &str) {
    assert!(
        !fixture.debug_smoke_output.exists() && !fixture.release_smoke_output.exists(),
        "{context} should not emit test artifacts"
    );
}

#[test]
fn test_package_path_json_uses_manifest_default_release_profile() {
    if !toolchain_available("`ql test --json` manifest default profile test") {
        return;
    }

    let workspace_root = workspace_root();
    let fixture = write_package_default_release_profile_fixture(
        "ql-project-test-json-manifest-default-profile",
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command.args(["test", "--json"]).arg(&fixture.project_root);
    let output = run_command_capture(&mut command, "`ql test --json` manifest default profile");
    let (stdout, stderr) = expect_success(
        "project-test-json-manifest-default-profile",
        "manifest default profile test json success",
        &output,
    )
    .expect("package-path `ql test --json` should honor manifest default profile");
    expect_empty_stderr(
        "project-test-json-manifest-default-profile",
        "manifest default profile test json success",
        &stderr,
    )
    .expect("manifest default profile test json success should not print stderr");

    let expected =
        expected_package_profile_test_json(&fixture.project_root, "debug", false, "release");
    assert_profile_json(
        "project-test-json-manifest-default-profile",
        &stdout,
        expected,
        "package-path `ql test --json` should report requested and effective profiles separately",
    );
    expect_file_exists(
        "project-test-json-manifest-default-profile",
        &fixture.release_smoke_output,
        "manifest default profile json smoke executable",
        "manifest default profile test json success",
    )
    .expect("manifest default profile test json success should emit release artifact");
    assert!(
        !fixture.debug_smoke_output.exists(),
        "manifest default profile test json success should not silently emit debug artifacts"
    );
}

#[test]
fn test_package_path_json_profile_override_keeps_debug_profile() {
    if !toolchain_available("`ql test --json --profile debug` manifest profile override test") {
        return;
    }

    let workspace_root = workspace_root();
    let fixture =
        write_package_default_release_profile_fixture("ql-project-test-json-profile-override");

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["test", "--json", "--profile", "debug"])
        .arg(&fixture.project_root);
    let output = run_command_capture(
        &mut command,
        "`ql test --json --profile debug` manifest profile override",
    );
    let (stdout, stderr) = expect_success(
        "project-test-json-profile-override",
        "manifest profile override test json success",
        &output,
    )
    .expect(
        "package-path `ql test --json --profile debug` should override manifest default profile",
    );
    expect_empty_stderr(
        "project-test-json-profile-override",
        "manifest profile override test json success",
        &stderr,
    )
    .expect("manifest profile override test json success should not print stderr");

    let expected =
        expected_package_profile_test_json(&fixture.project_root, "debug", true, "debug");
    assert_profile_json(
        "project-test-json-profile-override",
        &stdout,
        expected,
        "package-path `ql test --json --profile debug` should report explicit profile override",
    );
    expect_file_exists(
        "project-test-json-profile-override",
        &fixture.debug_smoke_output,
        "manifest profile override json smoke executable",
        "manifest profile override test json success",
    )
    .expect("manifest profile override test json success should emit debug artifact");
    assert!(
        !fixture.release_smoke_output.exists(),
        "manifest profile override test json success should not emit release artifacts"
    );
}

#[test]
fn test_package_path_json_release_flag_overrides_manifest_default_profile() {
    if !toolchain_available("`ql test --json --release` manifest profile override test") {
        return;
    }

    let workspace_root = workspace_root();
    let fixture =
        write_package_default_debug_profile_fixture("ql-project-test-json-release-override");

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["test", "--json", "--release"])
        .arg(&fixture.project_root);
    let output = run_command_capture(
        &mut command,
        "`ql test --json --release` manifest profile override",
    );
    let (stdout, stderr) = expect_success(
        "project-test-json-release-override",
        "manifest release alias override test json success",
        &output,
    )
    .expect("package-path `ql test --json --release` should override manifest default profile");
    expect_empty_stderr(
        "project-test-json-release-override",
        "manifest release alias override test json success",
        &stderr,
    )
    .expect("manifest release alias override test json success should not print stderr");

    let expected =
        expected_package_profile_test_json(&fixture.project_root, "release", true, "release");
    assert_profile_json(
        "project-test-json-release-override",
        &stdout,
        expected,
        "package-path `ql test --json --release` should report explicit release profile override",
    );
    expect_file_exists(
        "project-test-json-release-override",
        &fixture.release_smoke_output,
        "manifest release alias override json smoke executable",
        "manifest release alias override test json success",
    )
    .expect("manifest release alias override test json success should emit release artifact");
    assert!(
        !fixture.debug_smoke_output.exists(),
        "manifest release alias override test json success should not emit debug artifact"
    );
}

#[test]
fn test_package_path_list_json_uses_manifest_default_release_profile() {
    let workspace_root = workspace_root();
    let fixture = write_package_default_release_profile_fixture(
        "ql-project-test-list-json-manifest-default-profile",
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["test", "--list", "--json"])
        .arg(&fixture.project_root);
    let output = run_command_capture(
        &mut command,
        "`ql test --list --json` manifest default profile",
    );
    let (stdout, stderr) = expect_success(
        "project-test-list-json-manifest-default-profile",
        "manifest default profile test list json",
        &output,
    )
    .expect("package-path `ql test --list --json` should honor manifest default profile");
    expect_empty_stderr(
        "project-test-list-json-manifest-default-profile",
        "manifest default profile test list json",
        &stderr,
    )
    .expect("manifest default profile test list json should not print stderr");

    let expected = expected_package_profile_test_listing_json(
        &fixture.project_root,
        "debug",
        false,
        "release",
    );
    assert_profile_json(
        "project-test-list-json-manifest-default-profile",
        &stdout,
        expected,
        "package-path `ql test --list --json` should expose the effective release profile",
    );
    assert_package_profile_artifacts_absent(&fixture, "manifest default profile list json");
}

#[test]
fn test_package_path_list_json_profile_override_keeps_debug_profile() {
    let workspace_root = workspace_root();
    let fixture =
        write_package_default_release_profile_fixture("ql-project-test-list-json-profile-override");

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["test", "--list", "--json", "--profile", "debug"])
        .arg(&fixture.project_root);
    let output = run_command_capture(
        &mut command,
        "`ql test --list --json --profile debug` manifest profile override",
    );
    let (stdout, stderr) = expect_success(
        "project-test-list-json-profile-override",
        "manifest profile override test list json",
        &output,
    )
    .expect("package-path `ql test --list --json --profile debug` should keep debug profile");
    expect_empty_stderr(
        "project-test-list-json-profile-override",
        "manifest profile override test list json",
        &stderr,
    )
    .expect("manifest profile override test list json should not print stderr");

    let expected =
        expected_package_profile_test_listing_json(&fixture.project_root, "debug", true, "debug");
    assert_profile_json(
        "project-test-list-json-profile-override",
        &stdout,
        expected,
        "package-path `ql test --list --json --profile debug` should expose the explicit debug profile",
    );
    assert_package_profile_artifacts_absent(&fixture, "manifest profile override list json");
}

#[test]
fn test_direct_project_test_file_json_uses_manifest_default_release_profile() {
    if !toolchain_available("`ql test --json` direct project file manifest profile test") {
        return;
    }

    let workspace_root = workspace_root();
    let fixture = write_package_default_release_profile_fixture(
        "ql-project-test-direct-file-json-manifest-default-profile",
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command.args(["test", "--json"]).arg(&fixture.smoke_path);
    let output = run_command_capture(
        &mut command,
        "`ql test --json` direct project file manifest default profile",
    );
    let (stdout, stderr) = expect_success(
        "project-test-direct-file-json-manifest-default-profile",
        "direct project file manifest default profile json success",
        &output,
    )
    .expect("direct project test file `ql test --json` should honor manifest default profile");
    expect_empty_stderr(
        "project-test-direct-file-json-manifest-default-profile",
        "direct project file manifest default profile json success",
        &stderr,
    )
    .expect("direct project file manifest default profile json success should not print stderr");

    let expected =
        expected_package_profile_test_json(&fixture.smoke_path, "debug", false, "release");
    assert_profile_json(
        "project-test-direct-file-json-manifest-default-profile",
        &stdout,
        expected,
        "direct project test file json should expose the effective release profile",
    );
    expect_file_exists(
        "project-test-direct-file-json-manifest-default-profile",
        &fixture.release_smoke_output,
        "direct project file manifest default profile smoke executable",
        "direct project file manifest default profile json success",
    )
    .expect(
        "direct project file manifest default profile json success should emit release artifact",
    );
    assert!(
        !fixture.debug_smoke_output.exists(),
        "direct project file manifest default profile json success should not emit debug artifact"
    );
}

#[test]
fn test_direct_project_test_file_json_profile_override_keeps_debug_profile() {
    if !toolchain_available(
        "`ql test --json --profile debug` direct project file manifest profile override test",
    ) {
        return;
    }

    let workspace_root = workspace_root();
    let fixture = write_package_default_release_profile_fixture(
        "ql-project-test-direct-file-json-profile-override",
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["test", "--json", "--profile", "debug"])
        .arg(&fixture.smoke_path);
    let output = run_command_capture(
        &mut command,
        "`ql test --json --profile debug` direct project file manifest profile override",
    );
    let (stdout, stderr) = expect_success(
        "project-test-direct-file-json-profile-override",
        "direct project file manifest profile override json success",
        &output,
    )
    .expect(
        "direct project test file `ql test --json --profile debug` should override manifest default profile",
    );
    expect_empty_stderr(
        "project-test-direct-file-json-profile-override",
        "direct project file manifest profile override json success",
        &stderr,
    )
    .expect("direct project file manifest profile override json success should not print stderr");

    let expected = expected_package_profile_test_json(&fixture.smoke_path, "debug", true, "debug");
    assert_profile_json(
        "project-test-direct-file-json-profile-override",
        &stdout,
        expected,
        "direct project test file json should expose the explicit debug profile",
    );
    expect_file_exists(
        "project-test-direct-file-json-profile-override",
        &fixture.debug_smoke_output,
        "direct project file manifest profile override smoke executable",
        "direct project file manifest profile override json success",
    )
    .expect(
        "direct project file manifest profile override json success should emit debug artifact",
    );
    assert!(
        !fixture.release_smoke_output.exists(),
        "direct project file manifest profile override json success should not emit release artifact"
    );
}

#[test]
fn test_direct_project_test_file_json_release_flag_overrides_manifest_default_profile() {
    if !toolchain_available(
        "`ql test --json --release` direct project file manifest profile override test",
    ) {
        return;
    }

    let workspace_root = workspace_root();
    let fixture = write_package_default_debug_profile_fixture(
        "ql-project-test-direct-file-json-release-override",
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["test", "--json", "--release"])
        .arg(&fixture.smoke_path);
    let output = run_command_capture(
        &mut command,
        "`ql test --json --release` direct project file manifest profile override",
    );
    let (stdout, stderr) = expect_success(
        "project-test-direct-file-json-release-override",
        "direct project file manifest release alias override json success",
        &output,
    )
    .expect(
        "direct project test file `ql test --json --release` should override manifest default profile",
    );
    expect_empty_stderr(
        "project-test-direct-file-json-release-override",
        "direct project file manifest release alias override json success",
        &stderr,
    )
    .expect(
        "direct project file manifest release alias override json success should not print stderr",
    );

    let expected =
        expected_package_profile_test_json(&fixture.smoke_path, "release", true, "release");
    assert_profile_json(
        "project-test-direct-file-json-release-override",
        &stdout,
        expected,
        "direct project test file json should expose the explicit release profile",
    );
    expect_file_exists(
        "project-test-direct-file-json-release-override",
        &fixture.release_smoke_output,
        "direct project file manifest release alias override smoke executable",
        "direct project file manifest release alias override json success",
    )
    .expect(
        "direct project file manifest release alias override json success should emit release artifact",
    );
    assert!(
        !fixture.debug_smoke_output.exists(),
        "direct project file manifest release alias override json success should not emit debug artifact"
    );
}

#[test]
fn test_direct_project_test_file_release_flag_overrides_manifest_default_profile() {
    if !toolchain_available(
        "`ql test --release` direct project file manifest profile override test",
    ) {
        return;
    }

    let workspace_root = workspace_root();
    let fixture = write_package_default_debug_profile_fixture("ql-project-test-release-override");

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["test"])
        .arg(&fixture.smoke_path)
        .arg("--release");
    let output = run_command_capture(&mut command, "`ql test --release` direct project file");
    let (stdout, stderr) = expect_success(
        "project-test-release-override",
        "direct project file release alias test",
        &output,
    )
    .expect(
        "direct project test file `ql test --release` should override manifest default profile",
    );
    expect_empty_stderr(
        "project-test-release-override",
        "direct project file release alias test",
        &stderr,
    )
    .expect("direct project file release alias test should not print stderr");
    expect_profile_stdout(
        "project-test-release-override",
        &stdout,
        "test tests/smoke.ql ... ok",
    );
    expect_file_exists(
        "project-test-release-override",
        &fixture.release_smoke_output,
        "direct project file release alias smoke executable",
        "direct project file release alias test",
    )
    .expect(
        "direct project file release alias test should emit smoke test artifacts under release",
    );
    assert!(
        !fixture.debug_smoke_output.exists(),
        "direct project file release alias test should not emit debug artifacts"
    );
}

#[test]
fn test_direct_project_test_file_list_json_uses_manifest_default_release_profile() {
    let workspace_root = workspace_root();
    let fixture = write_package_default_release_profile_fixture(
        "ql-project-test-direct-file-list-json-manifest-default-profile",
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["test", "--list", "--json"])
        .arg(&fixture.smoke_path);
    let output = run_command_capture(
        &mut command,
        "`ql test --list --json` direct project file manifest default profile",
    );
    let (stdout, stderr) = expect_success(
        "project-test-direct-file-list-json-manifest-default-profile",
        "direct project file manifest default profile list json",
        &output,
    )
    .expect(
        "direct project test file `ql test --list --json` should honor manifest default profile",
    );
    expect_empty_stderr(
        "project-test-direct-file-list-json-manifest-default-profile",
        "direct project file manifest default profile list json",
        &stderr,
    )
    .expect("direct project file manifest default profile list json should not print stderr");

    let expected =
        expected_package_profile_test_listing_json(&fixture.smoke_path, "debug", false, "release");
    assert_profile_json(
        "project-test-direct-file-list-json-manifest-default-profile",
        &stdout,
        expected,
        "direct project test file list json should expose the effective release profile",
    );
    assert_package_profile_artifacts_absent(
        &fixture,
        "direct project file manifest default profile list json",
    );
}

#[test]
fn test_direct_project_test_file_list_json_profile_override_keeps_debug_profile() {
    let workspace_root = workspace_root();
    let fixture = write_package_default_release_profile_fixture(
        "ql-project-test-direct-file-list-json-profile-override",
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["test", "--list", "--json", "--profile", "debug"])
        .arg(&fixture.smoke_path);
    let output = run_command_capture(
        &mut command,
        "`ql test --list --json --profile debug` direct project file manifest profile override",
    );
    let (stdout, stderr) = expect_success(
        "project-test-direct-file-list-json-profile-override",
        "direct project file manifest profile override list json",
        &output,
    )
    .expect("direct project test file `ql test --list --json --profile debug` should keep debug profile");
    expect_empty_stderr(
        "project-test-direct-file-list-json-profile-override",
        "direct project file manifest profile override list json",
        &stderr,
    )
    .expect("direct project file manifest profile override list json should not print stderr");

    let expected =
        expected_package_profile_test_listing_json(&fixture.smoke_path, "debug", true, "debug");
    assert_profile_json(
        "project-test-direct-file-list-json-profile-override",
        &stdout,
        expected,
        "direct project test file list json should expose the explicit debug profile",
    );
    assert_package_profile_artifacts_absent(
        &fixture,
        "direct project file manifest profile override list json",
    );
}

#[test]
fn test_workspace_path_json_uses_workspace_default_release_profile() {
    if !toolchain_available("`ql test --json` workspace default profile test") {
        return;
    }

    let workspace_root = workspace_root();
    let fixture = write_workspace_default_release_profile_fixture(
        "ql-project-test-json-workspace-default-profile",
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command.args(["test", "--json"]).arg(&fixture.project_root);
    let output = run_command_capture(&mut command, "`ql test --json` workspace default profile");
    let (stdout, stderr) = expect_success(
        "project-test-json-workspace-default-profile",
        "workspace default profile test json success",
        &output,
    )
    .expect("workspace-path `ql test --json` should honor workspace default profile");
    expect_empty_stderr(
        "project-test-json-workspace-default-profile",
        "workspace default profile test json success",
        &stderr,
    )
    .expect("workspace default profile test json success should not print stderr");

    let expected =
        expected_workspace_profile_test_json(&fixture.project_root, "debug", false, "release");
    assert_profile_json(
        "project-test-json-workspace-default-profile",
        &stdout,
        expected,
        "workspace-path `ql test --json` should report requested and effective profiles separately",
    );
    expect_file_exists(
        "project-test-json-workspace-default-profile",
        &fixture.release_smoke_output,
        "workspace default profile json smoke executable",
        "workspace default profile test json success",
    )
    .expect("workspace default profile test json success should emit release artifact");
    assert!(
        !fixture.debug_smoke_output.exists(),
        "workspace default profile test json success should not silently emit debug artifacts"
    );
}

#[test]
fn test_workspace_path_json_profile_override_keeps_debug_profile() {
    if !toolchain_available("`ql test --json --profile debug` workspace profile override test") {
        return;
    }

    let workspace_root = workspace_root();
    let fixture = write_workspace_default_release_profile_fixture(
        "ql-project-test-json-workspace-profile-override",
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["test", "--json", "--profile", "debug"])
        .arg(&fixture.project_root);
    let output = run_command_capture(
        &mut command,
        "`ql test --json --profile debug` workspace profile override",
    );
    let (stdout, stderr) = expect_success(
        "project-test-json-workspace-profile-override",
        "workspace profile override test json success",
        &output,
    )
    .expect(
        "workspace-path `ql test --json --profile debug` should override workspace default profile",
    );
    expect_empty_stderr(
        "project-test-json-workspace-profile-override",
        "workspace profile override test json success",
        &stderr,
    )
    .expect("workspace profile override test json success should not print stderr");

    let expected =
        expected_workspace_profile_test_json(&fixture.project_root, "debug", true, "debug");
    assert_profile_json(
        "project-test-json-workspace-profile-override",
        &stdout,
        expected,
        "workspace-path `ql test --json --profile debug` should report explicit profile override",
    );
    expect_file_exists(
        "project-test-json-workspace-profile-override",
        &fixture.debug_smoke_output,
        "workspace profile override json smoke executable",
        "workspace profile override test json success",
    )
    .expect("workspace profile override test json success should emit debug artifact");
    assert!(
        !fixture.release_smoke_output.exists(),
        "workspace profile override test json success should not emit release artifacts"
    );
}

#[test]
fn test_workspace_path_json_release_flag_overrides_workspace_default_profile() {
    if !toolchain_available("`ql test --json --release` workspace profile override test") {
        return;
    }

    let workspace_root = workspace_root();
    let fixture = write_workspace_default_debug_profile_fixture(
        "ql-project-test-json-workspace-release-override",
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["test", "--json", "--release"])
        .arg(&fixture.project_root);
    let output = run_command_capture(
        &mut command,
        "`ql test --json --release` workspace profile override",
    );
    let (stdout, stderr) = expect_success(
        "project-test-json-workspace-release-override",
        "workspace release alias override test json success",
        &output,
    )
    .expect("workspace-path `ql test --json --release` should override workspace default profile");
    expect_empty_stderr(
        "project-test-json-workspace-release-override",
        "workspace release alias override test json success",
        &stderr,
    )
    .expect("workspace release alias override test json success should not print stderr");

    let expected =
        expected_workspace_profile_test_json(&fixture.project_root, "release", true, "release");
    assert_profile_json(
        "project-test-json-workspace-release-override",
        &stdout,
        expected,
        "workspace-path `ql test --json --release` should report explicit release profile override",
    );
    expect_file_exists(
        "project-test-json-workspace-release-override",
        &fixture.release_smoke_output,
        "workspace release alias override json smoke executable",
        "workspace release alias override test json success",
    )
    .expect("workspace release alias override test json success should emit release artifact");
    assert!(
        !fixture.debug_smoke_output.exists(),
        "workspace release alias override test json success should not emit debug artifact"
    );
}

#[test]
fn test_workspace_path_list_json_uses_workspace_default_release_profile() {
    let workspace_root = workspace_root();
    let fixture = write_workspace_default_release_profile_fixture(
        "ql-project-test-list-json-workspace-default-profile",
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["test", "--list", "--json"])
        .arg(&fixture.project_root);
    let output = run_command_capture(
        &mut command,
        "`ql test --list --json` workspace default profile",
    );
    let (stdout, stderr) = expect_success(
        "project-test-list-json-workspace-default-profile",
        "workspace default profile test list json",
        &output,
    )
    .expect("workspace-path `ql test --list --json` should honor workspace default profile");
    expect_empty_stderr(
        "project-test-list-json-workspace-default-profile",
        "workspace default profile test list json",
        &stderr,
    )
    .expect("workspace default profile test list json should not print stderr");

    let expected = expected_workspace_profile_test_listing_json(
        &fixture.project_root,
        "debug",
        false,
        "release",
    );
    assert_profile_json(
        "project-test-list-json-workspace-default-profile",
        &stdout,
        expected,
        "workspace-path `ql test --list --json` should expose the effective release profile",
    );
    assert_workspace_profile_artifacts_absent(&fixture, "workspace default profile list json");
}

#[test]
fn test_workspace_path_list_json_profile_override_keeps_debug_profile() {
    let workspace_root = workspace_root();
    let fixture = write_workspace_default_release_profile_fixture(
        "ql-project-test-list-json-workspace-profile-override",
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["test", "--list", "--json", "--profile", "debug"])
        .arg(&fixture.project_root);
    let output = run_command_capture(
        &mut command,
        "`ql test --list --json --profile debug` workspace profile override",
    );
    let (stdout, stderr) = expect_success(
        "project-test-list-json-workspace-profile-override",
        "workspace profile override test list json",
        &output,
    )
    .expect("workspace-path `ql test --list --json --profile debug` should keep debug profile");
    expect_empty_stderr(
        "project-test-list-json-workspace-profile-override",
        "workspace profile override test list json",
        &stderr,
    )
    .expect("workspace profile override test list json should not print stderr");

    let expected =
        expected_workspace_profile_test_listing_json(&fixture.project_root, "debug", true, "debug");
    assert_profile_json(
        "project-test-list-json-workspace-profile-override",
        &stdout,
        expected,
        "workspace-path `ql test --list --json --profile debug` should expose the explicit debug profile",
    );
    assert_workspace_profile_artifacts_absent(&fixture, "workspace profile override list json");
}

#[test]
fn test_workspace_member_file_json_uses_workspace_default_release_profile() {
    if !toolchain_available("`ql test --json` workspace member file profile test") {
        return;
    }

    let workspace_root = workspace_root();
    let fixture = write_workspace_default_release_profile_fixture(
        "ql-project-test-json-workspace-member-file-default-profile",
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command.args(["test", "--json"]).arg(&fixture.smoke_path);
    let output = run_command_capture(
        &mut command,
        "`ql test --json` workspace member file default profile",
    );
    let (stdout, stderr) = expect_success(
        "project-test-json-workspace-member-file-default-profile",
        "workspace member file default profile json success",
        &output,
    )
    .expect("workspace member file `ql test --json` should honor workspace default profile");
    expect_empty_stderr(
        "project-test-json-workspace-member-file-default-profile",
        "workspace member file default profile json success",
        &stderr,
    )
    .expect("workspace member file default profile json success should not print stderr");

    let expected =
        expected_workspace_profile_test_json(&fixture.smoke_path, "debug", false, "release");
    assert_profile_json(
        "project-test-json-workspace-member-file-default-profile",
        &stdout,
        expected,
        "workspace member file json should expose the effective release profile",
    );
    expect_file_exists(
        "project-test-json-workspace-member-file-default-profile",
        &fixture.release_smoke_output,
        "workspace member file default profile smoke executable",
        "workspace member file default profile json success",
    )
    .expect("workspace member file default profile json success should emit release artifact");
    assert!(
        !fixture.debug_smoke_output.exists(),
        "workspace member file default profile json success should not emit debug artifact"
    );
}

#[test]
fn test_workspace_member_file_json_profile_override_keeps_debug_profile() {
    if !toolchain_available(
        "`ql test --json --profile debug` workspace member file profile override test",
    ) {
        return;
    }

    let workspace_root = workspace_root();
    let fixture = write_workspace_default_release_profile_fixture(
        "ql-project-test-json-workspace-member-file-profile-override",
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["test", "--json", "--profile", "debug"])
        .arg(&fixture.smoke_path);
    let output = run_command_capture(
        &mut command,
        "`ql test --json --profile debug` workspace member file profile override",
    );
    let (stdout, stderr) = expect_success(
        "project-test-json-workspace-member-file-profile-override",
        "workspace member file profile override json success",
        &output,
    )
    .expect(
        "workspace member file `ql test --json --profile debug` should override workspace default profile",
    );
    expect_empty_stderr(
        "project-test-json-workspace-member-file-profile-override",
        "workspace member file profile override json success",
        &stderr,
    )
    .expect("workspace member file profile override json success should not print stderr");

    let expected =
        expected_workspace_profile_test_json(&fixture.smoke_path, "debug", true, "debug");
    assert_profile_json(
        "project-test-json-workspace-member-file-profile-override",
        &stdout,
        expected,
        "workspace member file json should expose the explicit debug profile",
    );
    expect_file_exists(
        "project-test-json-workspace-member-file-profile-override",
        &fixture.debug_smoke_output,
        "workspace member file profile override smoke executable",
        "workspace member file profile override json success",
    )
    .expect("workspace member file profile override json success should emit debug artifact");
    assert!(
        !fixture.release_smoke_output.exists(),
        "workspace member file profile override json success should not emit release artifact"
    );
}

#[test]
fn test_workspace_member_file_list_json_uses_workspace_default_release_profile() {
    let workspace_root = workspace_root();
    let fixture = write_workspace_default_release_profile_fixture(
        "ql-project-test-list-json-workspace-member-file-default-profile",
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["test", "--list", "--json"])
        .arg(&fixture.smoke_path);
    let output = run_command_capture(
        &mut command,
        "`ql test --list --json` workspace member file default profile",
    );
    let (stdout, stderr) = expect_success(
        "project-test-list-json-workspace-member-file-default-profile",
        "workspace member file default profile list json",
        &output,
    )
    .expect("workspace member file `ql test --list --json` should honor workspace default profile");
    expect_empty_stderr(
        "project-test-list-json-workspace-member-file-default-profile",
        "workspace member file default profile list json",
        &stderr,
    )
    .expect("workspace member file default profile list json should not print stderr");

    let expected = expected_workspace_profile_test_listing_json(
        &fixture.smoke_path,
        "debug",
        false,
        "release",
    );
    assert_profile_json(
        "project-test-list-json-workspace-member-file-default-profile",
        &stdout,
        expected,
        "workspace member file list json should expose the effective release profile",
    );
    assert_workspace_profile_artifacts_absent(
        &fixture,
        "workspace member file default profile list json",
    );
}

#[test]
fn test_workspace_member_file_list_json_profile_override_keeps_debug_profile() {
    let workspace_root = workspace_root();
    let fixture = write_workspace_default_release_profile_fixture(
        "ql-project-test-list-json-workspace-member-file-profile-override",
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["test", "--list", "--json", "--profile", "debug"])
        .arg(&fixture.smoke_path);
    let output = run_command_capture(
        &mut command,
        "`ql test --list --json --profile debug` workspace member file profile override",
    );
    let (stdout, stderr) = expect_success(
        "project-test-list-json-workspace-member-file-profile-override",
        "workspace member file profile override list json",
        &output,
    )
    .expect(
        "workspace member file `ql test --list --json --profile debug` should keep debug profile",
    );
    expect_empty_stderr(
        "project-test-list-json-workspace-member-file-profile-override",
        "workspace member file profile override list json",
        &stderr,
    )
    .expect("workspace member file profile override list json should not print stderr");

    let expected =
        expected_workspace_profile_test_listing_json(&fixture.smoke_path, "debug", true, "debug");
    assert_profile_json(
        "project-test-list-json-workspace-member-file-profile-override",
        &stdout,
        expected,
        "workspace member file list json should expose the explicit debug profile",
    );
    assert_workspace_profile_artifacts_absent(
        &fixture,
        "workspace member file profile override list json",
    );
}
