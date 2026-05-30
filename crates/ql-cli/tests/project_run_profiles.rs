mod support;

use ql_driver::{ToolchainOptions, discover_toolchain};
use serde_json::Value as JsonValue;
use support::{
    TempDir, executable_output_path, expect_empty_stderr, expect_exit_code, expect_file_exists,
    expect_silent_output, ql_command, run_command_capture, workspace_root,
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

struct PackageRunProfileFixture {
    temp: TempDir,
    project_root: std::path::PathBuf,
    manifest_path: std::path::PathBuf,
    debug_output: std::path::PathBuf,
    release_output: std::path::PathBuf,
}

fn write_package_run_profile_fixture(
    prefix: &str,
    default_profile: &str,
) -> PackageRunProfileFixture {
    let temp = TempDir::new(prefix);
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create package source tree for profile run test");
    let manifest = format!(
        r#"
[package]
name = "app"

[profile]
default = "{default_profile}"
"#,
    );
    let manifest_path = temp.write("app/qlang.toml", &manifest);
    temp.write("app/src/main.ql", "fn main() -> Int { return 13 }\n");

    let debug_output = executable_output_path(&project_root.join("target/ql/debug"), "main");
    let release_output = executable_output_path(&project_root.join("target/ql/release"), "main");

    PackageRunProfileFixture {
        temp,
        project_root,
        manifest_path,
        debug_output,
        release_output,
    }
}

fn write_package_default_release_run_fixture(prefix: &str) -> PackageRunProfileFixture {
    write_package_run_profile_fixture(prefix, "release")
}

fn write_package_default_debug_run_fixture(prefix: &str) -> PackageRunProfileFixture {
    write_package_run_profile_fixture(prefix, "debug")
}

struct WorkspaceRunProfileFixture {
    temp: TempDir,
    project_root: std::path::PathBuf,
    workspace_manifest: std::path::PathBuf,
    app_manifest: std::path::PathBuf,
    debug_output: std::path::PathBuf,
    release_output: std::path::PathBuf,
}

fn write_workspace_run_profile_fixture(
    prefix: &str,
    default_profile: &str,
) -> WorkspaceRunProfileFixture {
    let temp = TempDir::new(prefix);
    let project_root = temp.path().join("workspace");
    std::fs::create_dir_all(project_root.join("packages/app/src"))
        .expect("create workspace package source tree for profile run test");
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
        "workspace/packages/app/src/main.ql",
        "fn main() -> Int { return 13 }\n",
    );

    let debug_output =
        executable_output_path(&project_root.join("packages/app/target/ql/debug"), "main");
    let release_output =
        executable_output_path(&project_root.join("packages/app/target/ql/release"), "main");

    WorkspaceRunProfileFixture {
        temp,
        project_root,
        workspace_manifest,
        app_manifest,
        debug_output,
        release_output,
    }
}

fn write_workspace_default_release_run_fixture(prefix: &str) -> WorkspaceRunProfileFixture {
    write_workspace_run_profile_fixture(prefix, "release")
}

fn write_workspace_default_debug_run_fixture(prefix: &str) -> WorkspaceRunProfileFixture {
    write_workspace_run_profile_fixture(prefix, "debug")
}

fn expect_default_release_run_profile_json(
    case_name: &str,
    json: &JsonValue,
    command_path: &std::path::PathBuf,
    project_manifest_path: &std::path::PathBuf,
    target_manifest_path: &std::path::PathBuf,
    requested_profile: &str,
    profile_overridden: bool,
    target_profile: &str,
    artifact_path: &std::path::PathBuf,
) {
    assert_eq!(json["schema"], "ql.run.v1");
    assert_eq!(
        json["path"],
        command_path.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["scope"], "project");
    assert_eq!(
        json["project_manifest_path"],
        project_manifest_path
            .display()
            .to_string()
            .replace('\\', "/")
    );
    assert_eq!(json["requested_profile"], requested_profile);
    assert_eq!(json["profile_overridden"], profile_overridden);
    assert_eq!(json["program_args"], serde_json::json!([]));
    assert_eq!(json["status"], "completed");
    assert_eq!(json["failure"], JsonValue::Null);
    assert_eq!(
        json["built_target"],
        serde_json::json!({
            "manifest_path": target_manifest_path.display().to_string().replace('\\', "/"),
            "package_name": "app",
            "selected": true,
            "dependency_only": false,
            "kind": "bin",
            "path": "src/main.ql",
            "emit": "exe",
            "profile": target_profile,
            "artifact_path": artifact_path.display().to_string().replace('\\', "/"),
            "c_header_path": JsonValue::Null,
        }),
        "{case_name} should report the effective executable profile"
    );
    assert_eq!(
        json["execution"],
        serde_json::json!({
            "exit_code": 13,
            "stdout": "",
            "stderr": "",
        })
    );
}

#[test]
fn run_project_source_file_uses_project_aware_target_and_profile() {
    if !toolchain_available("`ql run` direct project source file test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-source-file-project-aware");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src/bin"))
        .expect("create package source tree for direct project source run test");
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"

[profile]
default = "release"
"#,
    );
    let main_path = temp.write("app/src/main.ql", "fn main() -> Int { return 13 }\n");
    temp.write("app/src/bin/admin.ql", "fn main() -> Int { return 2 }\n");
    let output_path = executable_output_path(&project_root.join("target/ql/release"), "main");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["run"]).arg(&main_path);
    let output = run_command_capture(&mut command, "`ql run` direct project source file");
    let (stdout, stderr) = expect_exit_code(
        "project-run-source-file-project-aware",
        "direct project source file run",
        &output,
        13,
    )
    .expect("direct project source file `ql run` should execute the selected target");
    expect_silent_output(
        "project-run-source-file-project-aware",
        "direct project source file run",
        &stdout,
        &stderr,
    )
    .expect("direct project source file `ql run` should leave stdout/stderr to the program");
    expect_file_exists(
        "project-run-source-file-project-aware",
        &output_path,
        "direct project source executable",
        "direct project source file run",
    )
    .expect("direct project source file `ql run` should emit the executable under the package target dir");
}

#[test]
fn run_project_source_file_json_release_flag_overrides_manifest_default_profile() {
    if !toolchain_available("`ql run --json --release` direct project source file test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-source-file-json-release-override");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create package source tree for direct project source release run test");
    let manifest_path = temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"

[profile]
default = "debug"
"#,
    );
    let main_path = temp.write("app/src/main.ql", "fn main() -> Int { return 13 }\n");
    let release_output = executable_output_path(&project_root.join("target/ql/release"), "main");
    let debug_output = executable_output_path(&project_root.join("target/ql/debug"), "main");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["run", "--json", "--release"]).arg(&main_path);
    let output = run_command_capture(
        &mut command,
        "`ql run --json --release` direct project source file",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-run-source-file-json-release-override",
        "direct project source file release alias run json",
        &output,
        13,
    )
    .expect("direct project source file `ql run --json --release` should execute");
    expect_empty_stderr(
        "project-run-source-file-json-release-override",
        "direct project source file release alias run json",
        &stderr,
    )
    .expect("direct project source file release alias run json should not print stderr");

    let json = parse_json_output("project-run-source-file-json-release-override", &stdout);
    expect_default_release_run_profile_json(
        "project-run-source-file-json-release-override",
        &json,
        &main_path,
        &manifest_path,
        &manifest_path,
        "release",
        true,
        "release",
        &release_output,
    );
    expect_file_exists(
        "project-run-source-file-json-release-override",
        &release_output,
        "direct project source release alias executable",
        "direct project source file release alias run json",
    )
    .expect("direct project source release alias run json should emit release executable");
    assert!(
        !debug_output.exists(),
        "direct project source release alias run json should not emit debug executable"
    );
}

#[test]
fn run_project_source_file_release_flag_overrides_manifest_default_profile() {
    if !toolchain_available("`ql run --release` direct project source file test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-source-file-release-override");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src/bin"))
        .expect("create package source tree for direct project source release run test");
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    let main_path = temp.write("app/src/main.ql", "fn main() -> Int { return 13 }\n");
    temp.write("app/src/bin/admin.ql", "fn main() -> Int { return 2 }\n");
    let release_output = executable_output_path(&project_root.join("target/ql/release"), "main");
    let debug_output = executable_output_path(&project_root.join("target/ql/debug"), "main");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["run"]).arg(&main_path).arg("--release");
    let output = run_command_capture(
        &mut command,
        "`ql run --release` direct project source file",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-run-source-file-release-override",
        "direct project source file release alias run",
        &output,
        13,
    )
    .expect("direct project source file `ql run --release` should execute");
    expect_silent_output(
        "project-run-source-file-release-override",
        "direct project source file release alias run",
        &stdout,
        &stderr,
    )
    .expect(
        "direct project source file release alias run should leave stdout/stderr to the program",
    );
    expect_file_exists(
        "project-run-source-file-release-override",
        &release_output,
        "direct project source release alias executable",
        "direct project source file release alias run",
    )
    .expect("direct project source file release alias run should emit the executable under the release target dir");
    assert!(
        !debug_output.exists(),
        "direct project source file release alias run should not emit the debug executable"
    );
}

#[test]
fn run_package_path_uses_manifest_default_release_profile() {
    if !toolchain_available("`ql run` manifest profile test") {
        return;
    }

    let workspace_root = workspace_root();
    let fixture = write_package_default_release_run_fixture("ql-project-run-manifest-profile");

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command.args(["run"]).arg(&fixture.project_root);
    let output = run_command_capture(&mut command, "`ql run` manifest default profile");
    let (stdout, stderr) = expect_exit_code(
        "project-run-manifest-profile",
        "manifest default profile run",
        &output,
        13,
    )
    .expect("package-path `ql run` should honor the manifest default profile");
    expect_silent_output(
        "project-run-manifest-profile",
        "manifest default profile run",
        &stdout,
        &stderr,
    )
    .expect("manifest default profile run should leave stdout/stderr to the program");
    expect_file_exists(
        "project-run-manifest-profile",
        &fixture.release_output,
        "manifest default profile executable",
        "manifest default profile run",
    )
    .expect("manifest default profile run should emit the release executable");
}

#[test]
fn run_package_path_profile_flag_overrides_manifest_default_profile() {
    if !toolchain_available("`ql run --profile debug` manifest profile test") {
        return;
    }

    let workspace_root = workspace_root();
    let fixture = write_package_default_release_run_fixture("ql-project-run-profile-override");

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["run"])
        .arg(&fixture.project_root)
        .args(["--profile", "debug"]);
    let output = run_command_capture(&mut command, "`ql run --profile debug` manifest override");
    let (stdout, stderr) = expect_exit_code(
        "project-run-profile-override",
        "manifest profile override run",
        &output,
        13,
    )
    .expect("package-path `ql run --profile debug` should override manifest default");
    expect_silent_output(
        "project-run-profile-override",
        "manifest profile override run",
        &stdout,
        &stderr,
    )
    .expect("manifest profile override run should leave stdout/stderr to the program");
    expect_file_exists(
        "project-run-profile-override",
        &fixture.debug_output,
        "manifest profile override executable",
        "manifest profile override run",
    )
    .expect("manifest profile override run should emit debug executable");
    assert!(
        !fixture.release_output.exists(),
        "manifest profile override run should not emit release executable"
    );
}

#[test]
fn run_package_path_release_flag_overrides_manifest_default_profile() {
    if !toolchain_available("`ql run --release` manifest profile test") {
        return;
    }

    let workspace_root = workspace_root();
    let fixture = write_package_default_debug_run_fixture("ql-project-run-release-override");

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["run"])
        .arg(&fixture.project_root)
        .arg("--release");
    let output = run_command_capture(&mut command, "`ql run --release` manifest override");
    let (stdout, stderr) = expect_exit_code(
        "project-run-release-override",
        "manifest release alias override run",
        &output,
        13,
    )
    .expect("package-path `ql run --release` should override manifest default");
    expect_silent_output(
        "project-run-release-override",
        "manifest release alias override run",
        &stdout,
        &stderr,
    )
    .expect("manifest release alias override run should leave stdout/stderr to the program");
    expect_file_exists(
        "project-run-release-override",
        &fixture.release_output,
        "manifest release alias override executable",
        "manifest release alias override run",
    )
    .expect("manifest release alias override run should emit release executable");
    assert!(
        !fixture.debug_output.exists(),
        "manifest release alias override run should not emit debug executable"
    );
}

#[test]
fn run_package_path_json_uses_manifest_default_release_profile() {
    if !toolchain_available("`ql run --json` manifest profile test") {
        return;
    }

    let workspace_root = workspace_root();
    let fixture =
        write_package_default_release_run_fixture("ql-project-run-json-manifest-default-profile");

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["run"])
        .arg(&fixture.project_root)
        .arg("--json");
    let output = run_command_capture(&mut command, "`ql run --json` manifest default profile");
    let (stdout, stderr) = expect_exit_code(
        "project-run-json-manifest-default-profile",
        "manifest default profile run json",
        &output,
        13,
    )
    .expect("package-path `ql run --json` should honor manifest default profile");
    expect_empty_stderr(
        "project-run-json-manifest-default-profile",
        "manifest default profile run json",
        &stderr,
    )
    .expect("manifest default profile run json should not print stderr");

    let json = parse_json_output("project-run-json-manifest-default-profile", &stdout);
    expect_default_release_run_profile_json(
        "project-run-json-manifest-default-profile",
        &json,
        &fixture.project_root,
        &fixture.manifest_path,
        &fixture.manifest_path,
        "debug",
        false,
        "release",
        &fixture.release_output,
    );
    expect_file_exists(
        "project-run-json-manifest-default-profile",
        &fixture.release_output,
        "manifest default profile run json executable",
        "manifest default profile run json",
    )
    .expect("manifest default profile run json should emit release executable");
    assert!(
        !fixture.debug_output.exists(),
        "manifest default profile run json should not emit debug executable"
    );
}

#[test]
fn run_package_path_json_profile_flag_overrides_manifest_default_profile() {
    if !toolchain_available("`ql run --json --profile debug` manifest profile test") {
        return;
    }

    let workspace_root = workspace_root();
    let fixture = write_package_default_release_run_fixture("ql-project-run-json-profile-override");

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["run"])
        .arg(&fixture.project_root)
        .args(["--json", "--profile", "debug"]);
    let output = run_command_capture(
        &mut command,
        "`ql run --json --profile debug` manifest override",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-run-json-profile-override",
        "manifest profile override run json",
        &output,
        13,
    )
    .expect("package-path `ql run --json --profile debug` should override manifest default");
    expect_empty_stderr(
        "project-run-json-profile-override",
        "manifest profile override run json",
        &stderr,
    )
    .expect("manifest profile override run json should not print stderr");

    let json = parse_json_output("project-run-json-profile-override", &stdout);
    expect_default_release_run_profile_json(
        "project-run-json-profile-override",
        &json,
        &fixture.project_root,
        &fixture.manifest_path,
        &fixture.manifest_path,
        "debug",
        true,
        "debug",
        &fixture.debug_output,
    );
    expect_file_exists(
        "project-run-json-profile-override",
        &fixture.debug_output,
        "manifest profile override run json executable",
        "manifest profile override run json",
    )
    .expect("manifest profile override run json should emit debug executable");
    assert!(
        !fixture.release_output.exists(),
        "manifest profile override run json should not emit release executable"
    );
}

#[test]
fn run_package_path_json_release_flag_overrides_manifest_default_profile() {
    if !toolchain_available("`ql run --json --release` manifest profile test") {
        return;
    }

    let workspace_root = workspace_root();
    let fixture = write_package_default_debug_run_fixture("ql-project-run-json-release-override");

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["run"])
        .arg(&fixture.project_root)
        .args(["--json", "--release"]);
    let output = run_command_capture(&mut command, "`ql run --json --release` manifest override");
    let (stdout, stderr) = expect_exit_code(
        "project-run-json-release-override",
        "manifest release alias override run json",
        &output,
        13,
    )
    .expect("package-path `ql run --json --release` should override manifest default");
    expect_empty_stderr(
        "project-run-json-release-override",
        "manifest release alias override run json",
        &stderr,
    )
    .expect("manifest release alias override run json should not print stderr");

    let json = parse_json_output("project-run-json-release-override", &stdout);
    expect_default_release_run_profile_json(
        "project-run-json-release-override",
        &json,
        &fixture.project_root,
        &fixture.manifest_path,
        &fixture.manifest_path,
        "release",
        true,
        "release",
        &fixture.release_output,
    );
    expect_file_exists(
        "project-run-json-release-override",
        &fixture.release_output,
        "manifest release alias override run json executable",
        "manifest release alias override run json",
    )
    .expect("manifest release alias override run json should emit release executable");
    assert!(
        !fixture.debug_output.exists(),
        "manifest release alias override run json should not emit debug executable"
    );
}

#[test]
fn run_workspace_path_uses_workspace_default_profile() {
    if !toolchain_available("`ql run` workspace profile test") {
        return;
    }

    let workspace_root = workspace_root();
    let fixture = write_workspace_default_release_run_fixture("ql-project-run-workspace-profile");

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command.args(["run"]).arg(&fixture.project_root);
    let output = run_command_capture(&mut command, "`ql run` workspace default profile");
    let (stdout, stderr) = expect_exit_code(
        "project-run-workspace-profile",
        "workspace default profile run",
        &output,
        13,
    )
    .expect("workspace-path `ql run` should honor the workspace default profile");
    expect_silent_output(
        "project-run-workspace-profile",
        "workspace default profile run",
        &stdout,
        &stderr,
    )
    .expect("workspace default profile run should leave stdout/stderr to the program");
    expect_file_exists(
        "project-run-workspace-profile",
        &fixture.release_output,
        "workspace default profile executable",
        "workspace default profile run",
    )
    .expect("workspace default profile run should emit the release executable");
}

#[test]
fn run_workspace_path_profile_flag_overrides_workspace_default_profile() {
    if !toolchain_available("`ql run --profile debug` workspace profile test") {
        return;
    }

    let workspace_root = workspace_root();
    let fixture =
        write_workspace_default_release_run_fixture("ql-project-run-workspace-profile-override");

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["run"])
        .arg(&fixture.project_root)
        .args(["--profile", "debug"]);
    let output = run_command_capture(&mut command, "`ql run --profile debug` workspace override");
    let (stdout, stderr) = expect_exit_code(
        "project-run-workspace-profile-override",
        "workspace profile override run",
        &output,
        13,
    )
    .expect("workspace-path `ql run --profile debug` should override workspace default");
    expect_silent_output(
        "project-run-workspace-profile-override",
        "workspace profile override run",
        &stdout,
        &stderr,
    )
    .expect("workspace profile override run should leave stdout/stderr to the program");
    expect_file_exists(
        "project-run-workspace-profile-override",
        &fixture.debug_output,
        "workspace profile override executable",
        "workspace profile override run",
    )
    .expect("workspace profile override run should emit debug executable");
    assert!(
        !fixture.release_output.exists(),
        "workspace profile override run should not emit release executable"
    );
}

#[test]
fn run_workspace_path_release_flag_overrides_workspace_default_profile() {
    if !toolchain_available("`ql run --release` workspace profile test") {
        return;
    }

    let workspace_root = workspace_root();
    let fixture =
        write_workspace_default_debug_run_fixture("ql-project-run-workspace-release-override");

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["run"])
        .arg(&fixture.project_root)
        .arg("--release");
    let output = run_command_capture(&mut command, "`ql run --release` workspace override");
    let (stdout, stderr) = expect_exit_code(
        "project-run-workspace-release-override",
        "workspace release alias override run",
        &output,
        13,
    )
    .expect("workspace-path `ql run --release` should override workspace default");
    expect_silent_output(
        "project-run-workspace-release-override",
        "workspace release alias override run",
        &stdout,
        &stderr,
    )
    .expect("workspace release alias override run should leave stdout/stderr to the program");
    expect_file_exists(
        "project-run-workspace-release-override",
        &fixture.release_output,
        "workspace release alias override executable",
        "workspace release alias override run",
    )
    .expect("workspace release alias override run should emit release executable");
    assert!(
        !fixture.debug_output.exists(),
        "workspace release alias override run should not emit debug executable"
    );
}

#[test]
fn run_workspace_path_json_uses_workspace_default_release_profile() {
    if !toolchain_available("`ql run --json` workspace profile test") {
        return;
    }

    let workspace_root = workspace_root();
    let fixture = write_workspace_default_release_run_fixture(
        "ql-project-run-json-workspace-default-profile",
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["run"])
        .arg(&fixture.project_root)
        .arg("--json");
    let output = run_command_capture(&mut command, "`ql run --json` workspace default profile");
    let (stdout, stderr) = expect_exit_code(
        "project-run-json-workspace-default-profile",
        "workspace default profile run json",
        &output,
        13,
    )
    .expect("workspace-path `ql run --json` should honor workspace default profile");
    expect_empty_stderr(
        "project-run-json-workspace-default-profile",
        "workspace default profile run json",
        &stderr,
    )
    .expect("workspace default profile run json should not print stderr");

    let json = parse_json_output("project-run-json-workspace-default-profile", &stdout);
    expect_default_release_run_profile_json(
        "project-run-json-workspace-default-profile",
        &json,
        &fixture.project_root,
        &fixture.workspace_manifest,
        &fixture.app_manifest,
        "debug",
        false,
        "release",
        &fixture.release_output,
    );
    expect_file_exists(
        "project-run-json-workspace-default-profile",
        &fixture.release_output,
        "workspace default profile run json executable",
        "workspace default profile run json",
    )
    .expect("workspace default profile run json should emit release executable");
    assert!(
        !fixture.debug_output.exists(),
        "workspace default profile run json should not emit debug executable"
    );
}

#[test]
fn run_workspace_path_json_profile_flag_overrides_workspace_default_profile() {
    if !toolchain_available("`ql run --json --profile debug` workspace profile test") {
        return;
    }

    let workspace_root = workspace_root();
    let fixture = write_workspace_default_release_run_fixture(
        "ql-project-run-json-workspace-profile-override",
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["run"])
        .arg(&fixture.project_root)
        .args(["--json", "--profile", "debug"]);
    let output = run_command_capture(
        &mut command,
        "`ql run --json --profile debug` workspace override",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-run-json-workspace-profile-override",
        "workspace profile override run json",
        &output,
        13,
    )
    .expect("workspace-path `ql run --json --profile debug` should override workspace default");
    expect_empty_stderr(
        "project-run-json-workspace-profile-override",
        "workspace profile override run json",
        &stderr,
    )
    .expect("workspace profile override run json should not print stderr");

    let json = parse_json_output("project-run-json-workspace-profile-override", &stdout);
    expect_default_release_run_profile_json(
        "project-run-json-workspace-profile-override",
        &json,
        &fixture.project_root,
        &fixture.workspace_manifest,
        &fixture.app_manifest,
        "debug",
        true,
        "debug",
        &fixture.debug_output,
    );
    expect_file_exists(
        "project-run-json-workspace-profile-override",
        &fixture.debug_output,
        "workspace profile override run json executable",
        "workspace profile override run json",
    )
    .expect("workspace profile override run json should emit debug executable");
    assert!(
        !fixture.release_output.exists(),
        "workspace profile override run json should not emit release executable"
    );
}

#[test]
fn run_workspace_path_json_release_flag_overrides_workspace_default_profile() {
    if !toolchain_available("`ql run --json --release` workspace profile test") {
        return;
    }

    let workspace_root = workspace_root();
    let fixture =
        write_workspace_default_debug_run_fixture("ql-project-run-json-workspace-release-override");

    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command
        .args(["run"])
        .arg(&fixture.project_root)
        .args(["--json", "--release"]);
    let output = run_command_capture(&mut command, "`ql run --json --release` workspace override");
    let (stdout, stderr) = expect_exit_code(
        "project-run-json-workspace-release-override",
        "workspace release alias override run json",
        &output,
        13,
    )
    .expect("workspace-path `ql run --json --release` should override workspace default");
    expect_empty_stderr(
        "project-run-json-workspace-release-override",
        "workspace release alias override run json",
        &stderr,
    )
    .expect("workspace release alias override run json should not print stderr");

    let json = parse_json_output("project-run-json-workspace-release-override", &stdout);
    expect_default_release_run_profile_json(
        "project-run-json-workspace-release-override",
        &json,
        &fixture.project_root,
        &fixture.workspace_manifest,
        &fixture.app_manifest,
        "release",
        true,
        "release",
        &fixture.release_output,
    );
    expect_file_exists(
        "project-run-json-workspace-release-override",
        &fixture.release_output,
        "workspace release alias override run json executable",
        "workspace release alias override run json",
    )
    .expect("workspace release alias override run json should emit release executable");
    assert!(
        !fixture.debug_output.exists(),
        "workspace release alias override run json should not emit debug executable"
    );
}

#[test]
fn run_workspace_member_source_file_uses_workspace_default_profile() {
    if !toolchain_available("`ql run` workspace source profile test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-workspace-source-profile");
    let project_root = temp.path().join("workspace");
    std::fs::create_dir_all(project_root.join("packages/app/src"))
        .expect("create workspace package source tree for workspace source profile run test");
    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app"]

[profile]
default = "release"
"#,
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    let main_path = temp.write(
        "workspace/packages/app/src/main.ql",
        "fn main() -> Int { return 17 }\n",
    );
    let output_path =
        executable_output_path(&project_root.join("packages/app/target/ql/release"), "main");
    let debug_output_path =
        executable_output_path(&project_root.join("packages/app/target/ql/debug"), "main");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["run"]).arg(&main_path);
    let output = run_command_capture(
        &mut command,
        "`ql run` workspace member source default profile",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-run-workspace-source-profile",
        "workspace member source default profile run",
        &output,
        17,
    )
    .expect("workspace member source path `ql run` should honor the workspace default profile");
    expect_silent_output(
        "project-run-workspace-source-profile",
        "workspace member source default profile run",
        &stdout,
        &stderr,
    )
    .expect(
        "workspace member source default profile run should leave stdout/stderr to the program",
    );
    expect_file_exists(
        "project-run-workspace-source-profile",
        &output_path,
        "workspace member source default profile executable",
        "workspace member source default profile run",
    )
    .expect("workspace member source default profile run should emit the release executable");
    assert!(
        !debug_output_path.exists(),
        "workspace member source default profile run should not silently fall back to the debug profile"
    );
}
