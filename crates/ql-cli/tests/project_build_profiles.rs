mod support;

use std::path::PathBuf;

use serde_json::Value as JsonValue;
use support::{
    TempDir, expect_empty_stderr, expect_file_exists, expect_stdout_contains_all, expect_success,
    ql_command, run_command_capture, static_library_output_path, workspace_root,
};

fn normalize_output_text(text: &str) -> String {
    text.replace("\r\n", "\n")
}

fn parse_json_output(case_name: &str, stdout: &str) -> JsonValue {
    serde_json::from_str(&normalize_output_text(stdout))
        .unwrap_or_else(|error| panic!("[{case_name}] parse json stdout: {error}\n{stdout}"))
}

struct PackageBuildProfileFixture {
    temp: TempDir,
    project_root: PathBuf,
    manifest_path: PathBuf,
    debug_output: PathBuf,
    release_output: PathBuf,
    interface_output: PathBuf,
}

fn write_package_build_profile_fixture(
    prefix: &str,
    default_profile: &str,
) -> PackageBuildProfileFixture {
    let temp = TempDir::new(prefix);
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create package source tree for profile build test");
    let manifest = format!(
        r#"
[package]
name = "app"

[profile]
default = "{default_profile}"
"#,
    );
    let manifest_path = temp.write("app/qlang.toml", &manifest);
    temp.write("app/src/lib.ql", "pub fn util() -> Int { return 1 }\n");

    let debug_output = static_library_output_path(&project_root.join("target/ql/debug"), "lib");
    let release_output = static_library_output_path(&project_root.join("target/ql/release"), "lib");
    let interface_output = project_root.join("app.qi");

    PackageBuildProfileFixture {
        temp,
        project_root,
        manifest_path,
        debug_output,
        release_output,
        interface_output,
    }
}

struct WorkspaceBuildProfileFixture {
    temp: TempDir,
    project_root: PathBuf,
    workspace_manifest: PathBuf,
    app_manifest: PathBuf,
    debug_output: PathBuf,
    release_output: PathBuf,
    interface_output: PathBuf,
}

fn write_workspace_build_profile_fixture(
    prefix: &str,
    default_profile: &str,
) -> WorkspaceBuildProfileFixture {
    let temp = TempDir::new(prefix);
    let project_root = temp.path().join("workspace");
    std::fs::create_dir_all(project_root.join("packages/app/src"))
        .expect("create workspace package source tree for profile build test");

    let workspace_manifest_text = format!(
        r#"
[workspace]
members = ["packages/app"]

[profile]
default = "{default_profile}"
"#,
    );
    let workspace_manifest = temp.write("workspace/qlang.toml", &workspace_manifest_text);
    let app_manifest = temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace/packages/app/src/lib.ql",
        "pub fn util() -> Int { return 1 }\n",
    );

    let debug_output =
        static_library_output_path(&project_root.join("packages/app/target/ql/debug"), "lib");
    let release_output =
        static_library_output_path(&project_root.join("packages/app/target/ql/release"), "lib");
    let interface_output = project_root.join("packages/app/app.qi");

    WorkspaceBuildProfileFixture {
        temp,
        project_root,
        workspace_manifest,
        app_manifest,
        debug_output,
        release_output,
        interface_output,
    }
}

fn write_workspace_default_release_build_fixture(prefix: &str) -> WorkspaceBuildProfileFixture {
    write_workspace_build_profile_fixture(prefix, "release")
}

fn write_workspace_default_debug_build_fixture(prefix: &str) -> WorkspaceBuildProfileFixture {
    write_workspace_build_profile_fixture(prefix, "debug")
}

fn write_package_default_release_build_fixture(prefix: &str) -> PackageBuildProfileFixture {
    write_package_build_profile_fixture(prefix, "release")
}

fn write_package_default_debug_build_fixture(prefix: &str) -> PackageBuildProfileFixture {
    write_package_build_profile_fixture(prefix, "debug")
}

struct BuildProfileJsonExpectation<'a> {
    command_path: &'a PathBuf,
    project_manifest_path: &'a PathBuf,
    target_manifest_path: &'a PathBuf,
    requested_profile: &'a str,
    profile_overridden: bool,
    target_profile: &'a str,
    artifact_path: &'a PathBuf,
    interface_output: Option<&'a PathBuf>,
}

fn expect_build_profile_json(
    case_name: &str,
    json: &JsonValue,
    expectation: BuildProfileJsonExpectation<'_>,
) {
    assert_eq!(json["schema"], "ql.build.v1");
    assert_eq!(
        json["path"],
        expectation
            .command_path
            .display()
            .to_string()
            .replace('\\', "/")
    );
    assert_eq!(json["scope"], "project");
    assert_eq!(
        json["project_manifest_path"],
        expectation
            .project_manifest_path
            .display()
            .to_string()
            .replace('\\', "/")
    );
    assert_eq!(json["requested_emit"], "llvm-ir");
    assert_eq!(json["requested_profile"], expectation.requested_profile);
    assert_eq!(json["profile_overridden"], expectation.profile_overridden);
    assert_eq!(json["emit_interface"], false);
    assert_eq!(json["status"], "ok");
    assert_eq!(json["failure"], JsonValue::Null);
    assert_eq!(
        json["built_targets"],
        serde_json::json!([
            {
                "manifest_path": expectation.target_manifest_path.display().to_string().replace('\\', "/"),
                "package_name": "app",
                "selected": true,
                "dependency_only": false,
                "kind": "lib",
                "path": "src/lib.ql",
                "emit": "staticlib",
                "profile": expectation.target_profile,
                "artifact_path": expectation.artifact_path.display().to_string().replace('\\', "/"),
                "c_header_path": JsonValue::Null,
            }
        ]),
        "{case_name} should report the effective artifact profile"
    );
    if let Some(interface_output) = expectation.interface_output {
        assert_eq!(
            json["interfaces"],
            serde_json::json!([
                {
                    "manifest_path": expectation.target_manifest_path.display().to_string().replace('\\', "/"),
                    "package_name": "app",
                    "selected": true,
                    "status": "wrote",
                    "path": interface_output.display().to_string().replace('\\', "/"),
                }
            ])
        );
    } else {
        assert_eq!(json["interfaces"], serde_json::json!([]));
    }
}

fn run_profile_build_text(
    fixture: &TempDir,
    project_root: &PathBuf,
    args: &[&str],
    command_name: &str,
    case_name: &str,
    description: &str,
) -> (String, String) {
    let workspace_root = workspace_root();
    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.path());
    command.args(["build"]).arg(project_root).args(args);
    let output = run_command_capture(&mut command, command_name);
    let (stdout, stderr) = expect_success(case_name, description, &output)
        .unwrap_or_else(|error| panic!("{description} should succeed: {error}"));
    expect_empty_stderr(case_name, description, &stderr)
        .unwrap_or_else(|error| panic!("{description} should not print stderr: {error}"));
    (stdout, stderr)
}

fn expect_profile_text_build(
    case_name: &str,
    description: &str,
    stdout: &str,
    output_path: &PathBuf,
    absent_output_path: Option<&PathBuf>,
) {
    expect_stdout_contains_all(
        case_name,
        &stdout.replace('\\', "/"),
        &[&format!("wrote staticlib: {}", output_path.display()).replace('\\', "/")],
    )
    .unwrap_or_else(|error| panic!("{description} should report the expected artifact: {error}"));
    expect_file_exists(case_name, output_path, description, description)
        .unwrap_or_else(|error| panic!("{description} should emit the expected artifact: {error}"));
    if let Some(absent_output_path) = absent_output_path {
        assert!(
            !absent_output_path.exists(),
            "{description} should not emit artifacts for the inactive profile"
        );
    }
}

fn run_profile_build_json(
    fixture: &TempDir,
    project_root: &PathBuf,
    args: &[&str],
    command_name: &str,
    case_name: &str,
    description: &str,
) -> JsonValue {
    let workspace_root = workspace_root();
    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.path());
    command.args(["build"]).arg(project_root).args(args);
    let output = run_command_capture(&mut command, command_name);
    let (stdout, stderr) = expect_success(case_name, description, &output)
        .unwrap_or_else(|error| panic!("{description} should succeed: {error}"));
    expect_empty_stderr(case_name, description, &stderr)
        .unwrap_or_else(|error| panic!("{description} should not print stderr: {error}"));
    parse_json_output(case_name, &stdout)
}

fn expect_json_artifact(
    case_name: &str,
    description: &str,
    output_path: &PathBuf,
    absent_output_path: Option<&PathBuf>,
) {
    expect_file_exists(case_name, output_path, description, description)
        .unwrap_or_else(|error| panic!("{description} should emit the expected artifact: {error}"));
    if let Some(absent_output_path) = absent_output_path {
        assert!(
            !absent_output_path.exists(),
            "{description} should not emit artifacts for the inactive profile"
        );
    }
}

#[test]
fn build_package_path_uses_manifest_default_release_profile() {
    let fixture = write_package_default_release_build_fixture("ql-project-build-manifest-profile");

    let (stdout, _) = run_profile_build_text(
        &fixture.temp,
        &fixture.project_root,
        &[],
        "`ql build` manifest default profile",
        "project-build-manifest-profile",
        "manifest default profile build",
    );
    expect_profile_text_build(
        "project-build-manifest-profile",
        "manifest default profile build",
        &stdout,
        &fixture.release_output,
        None,
    );
}

#[test]
fn build_package_path_profile_flag_overrides_manifest_default_profile() {
    let fixture = write_package_default_release_build_fixture("ql-project-build-profile-override");

    let (stdout, _) = run_profile_build_text(
        &fixture.temp,
        &fixture.project_root,
        &["--profile", "debug"],
        "`ql build --profile` manifest override",
        "project-build-profile-override",
        "manifest profile override build",
    );
    expect_profile_text_build(
        "project-build-profile-override",
        "manifest profile override build",
        &stdout,
        &fixture.debug_output,
        Some(&fixture.release_output),
    );
}

#[test]
fn build_package_path_release_flag_overrides_manifest_default_profile() {
    let fixture = write_package_default_debug_build_fixture("ql-project-build-release-override");

    let (stdout, _) = run_profile_build_text(
        &fixture.temp,
        &fixture.project_root,
        &["--release"],
        "`ql build --release` manifest override",
        "project-build-release-override",
        "manifest release alias override build",
    );
    expect_profile_text_build(
        "project-build-release-override",
        "manifest release alias override build",
        &stdout,
        &fixture.release_output,
        Some(&fixture.debug_output),
    );
}

#[test]
fn build_package_path_json_uses_manifest_default_release_profile() {
    let fixture = write_package_default_release_build_fixture(
        "ql-project-build-json-manifest-default-profile",
    );

    let json = run_profile_build_json(
        &fixture.temp,
        &fixture.project_root,
        &["--json"],
        "`ql build --json` manifest default profile",
        "project-build-json-manifest-default-profile",
        "manifest default profile build json",
    );
    expect_build_profile_json(
        "project-build-json-manifest-default-profile",
        &json,
        BuildProfileJsonExpectation {
            command_path: &fixture.project_root,
            project_manifest_path: &fixture.manifest_path,
            target_manifest_path: &fixture.manifest_path,
            requested_profile: "debug",
            profile_overridden: false,
            target_profile: "release",
            artifact_path: &fixture.release_output,
            interface_output: Some(&fixture.interface_output),
        },
    );
    expect_json_artifact(
        "project-build-json-manifest-default-profile",
        "manifest default profile build json",
        &fixture.release_output,
        Some(&fixture.debug_output),
    );
}

#[test]
fn build_package_path_json_profile_flag_overrides_manifest_default_profile() {
    let fixture =
        write_package_default_release_build_fixture("ql-project-build-json-profile-override");

    let json = run_profile_build_json(
        &fixture.temp,
        &fixture.project_root,
        &["--json", "--profile", "debug"],
        "`ql build --json --profile debug` manifest override",
        "project-build-json-profile-override",
        "manifest profile override build json",
    );
    expect_build_profile_json(
        "project-build-json-profile-override",
        &json,
        BuildProfileJsonExpectation {
            command_path: &fixture.project_root,
            project_manifest_path: &fixture.manifest_path,
            target_manifest_path: &fixture.manifest_path,
            requested_profile: "debug",
            profile_overridden: true,
            target_profile: "debug",
            artifact_path: &fixture.debug_output,
            interface_output: Some(&fixture.interface_output),
        },
    );
    expect_json_artifact(
        "project-build-json-profile-override",
        "manifest profile override build json",
        &fixture.debug_output,
        Some(&fixture.release_output),
    );
}

#[test]
fn build_package_path_json_release_flag_overrides_manifest_default_profile() {
    let fixture =
        write_package_default_debug_build_fixture("ql-project-build-json-release-override");

    let json = run_profile_build_json(
        &fixture.temp,
        &fixture.project_root,
        &["--json", "--release"],
        "`ql build --json --release` manifest override",
        "project-build-json-release-override",
        "manifest release alias override build json",
    );
    expect_build_profile_json(
        "project-build-json-release-override",
        &json,
        BuildProfileJsonExpectation {
            command_path: &fixture.project_root,
            project_manifest_path: &fixture.manifest_path,
            target_manifest_path: &fixture.manifest_path,
            requested_profile: "release",
            profile_overridden: true,
            target_profile: "release",
            artifact_path: &fixture.release_output,
            interface_output: Some(&fixture.interface_output),
        },
    );
    expect_json_artifact(
        "project-build-json-release-override",
        "manifest release alias override build json",
        &fixture.release_output,
        Some(&fixture.debug_output),
    );
}

#[test]
fn build_workspace_path_uses_workspace_default_profile() {
    let fixture =
        write_workspace_default_release_build_fixture("ql-project-build-workspace-profile");

    let (stdout, _) = run_profile_build_text(
        &fixture.temp,
        &fixture.project_root,
        &[],
        "`ql build` workspace default profile",
        "project-build-workspace-profile",
        "workspace default profile build",
    );
    expect_profile_text_build(
        "project-build-workspace-profile",
        "workspace default profile build",
        &stdout,
        &fixture.release_output,
        None,
    );
}

#[test]
fn build_workspace_path_profile_flag_overrides_workspace_default_profile() {
    let fixture = write_workspace_default_release_build_fixture(
        "ql-project-build-workspace-profile-override",
    );

    let (stdout, _) = run_profile_build_text(
        &fixture.temp,
        &fixture.project_root,
        &["--profile", "debug"],
        "`ql build --profile debug` workspace override",
        "project-build-workspace-profile-override",
        "workspace profile override build",
    );
    expect_profile_text_build(
        "project-build-workspace-profile-override",
        "workspace profile override build",
        &stdout,
        &fixture.debug_output,
        Some(&fixture.release_output),
    );
}

#[test]
fn build_workspace_path_release_flag_overrides_workspace_default_profile() {
    let fixture =
        write_workspace_default_debug_build_fixture("ql-project-build-workspace-release-override");

    let (stdout, _) = run_profile_build_text(
        &fixture.temp,
        &fixture.project_root,
        &["--release"],
        "`ql build --release` workspace override",
        "project-build-workspace-release-override",
        "workspace release alias override build",
    );
    expect_profile_text_build(
        "project-build-workspace-release-override",
        "workspace release alias override build",
        &stdout,
        &fixture.release_output,
        Some(&fixture.debug_output),
    );
}

#[test]
fn build_workspace_path_json_uses_workspace_default_release_profile() {
    let fixture = write_workspace_default_release_build_fixture(
        "ql-project-build-json-workspace-default-profile",
    );

    let json = run_profile_build_json(
        &fixture.temp,
        &fixture.project_root,
        &["--json"],
        "`ql build --json` workspace default profile",
        "project-build-json-workspace-default-profile",
        "workspace default profile build json",
    );
    expect_build_profile_json(
        "project-build-json-workspace-default-profile",
        &json,
        BuildProfileJsonExpectation {
            command_path: &fixture.project_root,
            project_manifest_path: &fixture.workspace_manifest,
            target_manifest_path: &fixture.app_manifest,
            requested_profile: "debug",
            profile_overridden: false,
            target_profile: "release",
            artifact_path: &fixture.release_output,
            interface_output: Some(&fixture.interface_output),
        },
    );
    expect_json_artifact(
        "project-build-json-workspace-default-profile",
        "workspace default profile build json",
        &fixture.release_output,
        Some(&fixture.debug_output),
    );
}

#[test]
fn build_workspace_path_json_profile_flag_overrides_workspace_default_profile() {
    let fixture = write_workspace_default_release_build_fixture(
        "ql-project-build-json-workspace-profile-override",
    );

    let json = run_profile_build_json(
        &fixture.temp,
        &fixture.project_root,
        &["--json", "--profile", "debug"],
        "`ql build --json --profile debug` workspace override",
        "project-build-json-workspace-profile-override",
        "workspace profile override build json",
    );
    expect_build_profile_json(
        "project-build-json-workspace-profile-override",
        &json,
        BuildProfileJsonExpectation {
            command_path: &fixture.project_root,
            project_manifest_path: &fixture.workspace_manifest,
            target_manifest_path: &fixture.app_manifest,
            requested_profile: "debug",
            profile_overridden: true,
            target_profile: "debug",
            artifact_path: &fixture.debug_output,
            interface_output: Some(&fixture.interface_output),
        },
    );
    expect_json_artifact(
        "project-build-json-workspace-profile-override",
        "workspace profile override build json",
        &fixture.debug_output,
        Some(&fixture.release_output),
    );
}

#[test]
fn build_workspace_path_json_release_flag_overrides_workspace_default_profile() {
    let fixture = write_workspace_default_debug_build_fixture(
        "ql-project-build-json-workspace-release-override",
    );

    let json = run_profile_build_json(
        &fixture.temp,
        &fixture.project_root,
        &["--json", "--release"],
        "`ql build --json --release` workspace override",
        "project-build-json-workspace-release-override",
        "workspace release alias override build json",
    );
    expect_build_profile_json(
        "project-build-json-workspace-release-override",
        &json,
        BuildProfileJsonExpectation {
            command_path: &fixture.project_root,
            project_manifest_path: &fixture.workspace_manifest,
            target_manifest_path: &fixture.app_manifest,
            requested_profile: "release",
            profile_overridden: true,
            target_profile: "release",
            artifact_path: &fixture.release_output,
            interface_output: Some(&fixture.interface_output),
        },
    );
    expect_json_artifact(
        "project-build-json-workspace-release-override",
        "workspace release alias override build json",
        &fixture.release_output,
        Some(&fixture.debug_output),
    );
}

#[test]
fn build_workspace_member_source_file_uses_workspace_default_profile() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-workspace-source-profile");
    let project_root = temp.path().join("workspace");
    std::fs::create_dir_all(project_root.join("packages/app/src"))
        .expect("create workspace package source tree for workspace source profile build test");

    let workspace_manifest = temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app"]

[profile]
default = "release"
"#,
    );
    let app_manifest = temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    let lib_path = temp.write(
        "workspace/packages/app/src/lib.ql",
        "pub fn util() -> Int { return 1 }\n",
    );

    let output_path =
        static_library_output_path(&project_root.join("packages/app/target/ql/release"), "lib");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["build"]).arg(&lib_path).arg("--json");
    let output = run_command_capture(
        &mut command,
        "`ql build --json` workspace member source default profile",
    );
    let (stdout, stderr) = expect_success(
        "project-build-workspace-source-profile",
        "workspace member source default profile build",
        &output,
    )
    .expect(
        "workspace member source path `ql build --json` should honor the workspace default profile",
    );
    expect_empty_stderr(
        "project-build-workspace-source-profile",
        "workspace member source default profile build",
        &stderr,
    )
    .expect("workspace member source default profile build should not print stderr");

    let json = parse_json_output("project-build-workspace-source-profile", &stdout);
    assert_eq!(json["schema"], "ql.build.v1");
    assert_eq!(
        json["path"],
        lib_path.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["scope"], "project");
    assert_eq!(
        json["project_manifest_path"],
        workspace_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["requested_emit"], "llvm-ir");
    assert_eq!(json["requested_profile"], "debug");
    assert_eq!(json["profile_overridden"], false);
    assert_eq!(json["emit_interface"], false);
    assert_eq!(json["status"], "ok");
    assert_eq!(json["failure"], JsonValue::Null);
    assert_eq!(
        json["interfaces"],
        serde_json::json!([
            {
                "manifest_path": app_manifest.display().to_string().replace('\\', "/"),
                "package_name": "app",
                "selected": true,
                "status": "wrote",
                "path": project_root.join("packages/app/app.qi").display().to_string().replace('\\', "/"),
            }
        ])
    );
    assert_eq!(
        json["built_targets"],
        serde_json::json!([
            {
                "manifest_path": app_manifest.display().to_string().replace('\\', "/"),
                "package_name": "app",
                "selected": true,
                "dependency_only": false,
                "kind": "lib",
                "path": "src/lib.ql",
                "emit": "staticlib",
                "profile": "release",
                "artifact_path": output_path.display().to_string().replace('\\', "/"),
                "c_header_path": JsonValue::Null,
            }
        ])
    );
    expect_file_exists(
        "project-build-workspace-source-profile",
        &output_path,
        "workspace member source default profile artifact",
        "workspace member source default profile build",
    )
    .expect("workspace member source default profile build should emit the release artifact");
}
