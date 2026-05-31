mod support;

use ql_driver::{ToolchainOptions, discover_toolchain};
use support::project_test_profiles::{
    assert_profile_json, assert_workspace_profile_artifacts_absent, expect_profile_stdout,
    expected_workspace_profile_test_json, expected_workspace_profile_test_listing_json,
    write_workspace_default_debug_profile_fixture, write_workspace_default_release_profile_fixture,
    write_workspace_member_file_profile_fixture,
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
