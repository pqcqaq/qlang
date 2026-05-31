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
