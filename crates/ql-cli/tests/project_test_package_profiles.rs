mod support;

use ql_driver::{ToolchainOptions, discover_toolchain};
use support::project_test_profiles::{
    assert_package_profile_artifacts_absent, assert_profile_json, expect_profile_stdout,
    expected_package_profile_test_json, expected_package_profile_test_listing_json,
    write_package_default_debug_profile_fixture, write_package_default_release_profile_fixture,
};
use support::{
    expect_empty_stderr, expect_file_exists, expect_success, ql_command, run_command_capture,
    workspace_root,
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
