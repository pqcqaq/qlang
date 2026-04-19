mod support;

use ql_driver::{ToolchainOptions, discover_toolchain};
use serde_json::Value as JsonValue;
use support::{
    TempDir, executable_output_path, expect_empty_stderr, expect_empty_stdout, expect_exit_code,
    expect_file_exists, expect_silent_output, expect_stderr_contains, expect_success, ql_command,
    run_command_capture, static_library_output_path, workspace_root,
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

#[test]
fn run_single_file_builds_and_executes_program() {
    if !toolchain_available("`ql run` single-file test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-file");
    let source_path = temp.write("demo.ql", "fn main() -> Int { return 7 }\n");
    let output_path = executable_output_path(&temp.path().join("target/ql/debug"), "demo");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["run"]).arg(&source_path);
    let output = run_command_capture(&mut command, "`ql run` single file");
    let (stdout, stderr) = expect_exit_code("project-run-file", "single-file run", &output, 7)
        .expect("single-file `ql run` should exit with the program status");
    expect_silent_output("project-run-file", "single-file run", &stdout, &stderr)
        .expect("single-file `ql run` should leave stdout/stderr to the program");
    expect_file_exists(
        "project-run-file",
        &output_path,
        "single-file executable",
        "single-file run",
    )
    .expect("single-file `ql run` should leave the built executable in the default path");
}

#[test]
fn run_single_file_supports_json_output() {
    if !toolchain_available("`ql run --json` single-file test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-file-json");
    let source_path = temp.write("demo.ql", "fn main() -> Int { return 7 }\n");
    let output_path = executable_output_path(&temp.path().join("target/ql/debug"), "demo");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["run"]).arg(&source_path).arg("--json");
    let output = run_command_capture(&mut command, "`ql run --json` single file");
    let (stdout, stderr) =
        expect_exit_code("project-run-file-json", "single-file run json", &output, 7)
            .expect("single-file `ql run --json` should preserve the program exit status");
    expect_empty_stderr("project-run-file-json", "single-file run json", &stderr)
        .expect("single-file `ql run --json` should keep stderr empty");

    let json = parse_json_output("project-run-file-json", &stdout);
    assert_eq!(json["schema"], "ql.run.v1");
    assert_eq!(
        json["path"],
        source_path.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["scope"], "file");
    assert_eq!(json["project_manifest_path"], JsonValue::Null);
    assert_eq!(json["requested_profile"], "debug");
    assert_eq!(json["profile_overridden"], false);
    assert_eq!(json["program_args"], serde_json::json!([]));
    assert_eq!(json["status"], "completed");
    assert_eq!(json["failure"], JsonValue::Null);
    assert_eq!(
        json["built_target"],
        serde_json::json!({
            "manifest_path": JsonValue::Null,
            "package_name": JsonValue::Null,
            "selected": true,
            "dependency_only": false,
            "kind": "source",
            "path": source_path.display().to_string().replace('\\', "/"),
            "emit": "exe",
            "profile": "debug",
            "artifact_path": output_path.display().to_string().replace('\\', "/"),
            "c_header_path": JsonValue::Null,
        })
    );
    assert_eq!(
        json["execution"],
        serde_json::json!({
            "exit_code": 7,
            "stdout": "",
            "stderr": "",
        })
    );
    expect_file_exists(
        "project-run-file-json",
        &output_path,
        "single-file run json executable",
        "single-file run json",
    )
    .expect(
        "single-file `ql run --json` should still leave the built executable in the default path",
    );
}

#[test]
fn run_single_file_supports_local_receiver_methods() {
    if !toolchain_available("`ql run` local receiver method test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-file-local-receiver-method");
    let source_path = temp.write(
        "demo.ql",
        "struct Box { value: Int }\n\nimpl Box {\n    fn read(self) -> Int {\n        return self.value\n    }\n}\n\nfn main() -> Int {\n    let value = Box { value: 7 }\n    return value.read()\n}\n",
    );
    let output_path = executable_output_path(&temp.path().join("target/ql/debug"), "demo");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["run"]).arg(&source_path);
    let output = run_command_capture(&mut command, "`ql run` local receiver method");
    let (stdout, stderr) = expect_exit_code(
        "project-run-file-local-receiver-method",
        "single-file local receiver method run",
        &output,
        7,
    )
    .expect("single-file `ql run` should execute local receiver methods");
    expect_silent_output(
        "project-run-file-local-receiver-method",
        "single-file local receiver method run",
        &stdout,
        &stderr,
    )
    .expect("single-file `ql run` local receiver method should leave stdout/stderr to the program");
    expect_file_exists(
        "project-run-file-local-receiver-method",
        &output_path,
        "single-file local receiver method executable",
        "single-file local receiver method run",
    )
    .expect("single-file `ql run` local receiver method should leave the built executable in the default path");
}

#[test]
fn run_package_path_executes_the_only_runnable_target_with_program_args() {
    if !toolchain_available("`ql run` package test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-package");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create package source tree for run test");
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write("app/src/lib.ql", "pub fn helper() -> Int { return 1 }\n");
    temp.write("app/src/main.ql", "fn main() -> Int { return 9 }\n");
    let output_path = executable_output_path(&project_root.join("target/ql/debug"), "main");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["run"])
        .arg(&project_root)
        .arg("--")
        .args(["alpha", "beta"]);
    let output = run_command_capture(&mut command, "`ql run` package path");
    let (stdout, stderr) = expect_exit_code("project-run-package", "package path run", &output, 9)
        .expect("package-path `ql run` should exit with the runnable target status");
    expect_silent_output("project-run-package", "package path run", &stdout, &stderr)
        .expect("package-path `ql run` should leave stdout/stderr to the program");
    expect_file_exists(
        "project-run-package",
        &output_path,
        "package executable",
        "package path run",
    )
    .expect("package-path `ql run` should leave the built executable in the package target dir");
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
fn run_project_path_json_reports_build_failure() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-project-json-build-failure");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create package source tree for run json build failure");
    let app_manifest = temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    let main_path = temp.write("app/src/main.ql", "fn main() -> Int { return \"oops\" }\n");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["run"]).arg(&project_root).arg("--json");
    let output = run_command_capture(&mut command, "`ql run --json` project build failure");
    let (stdout, stderr) = expect_exit_code(
        "project-run-project-json-build-failure",
        "project run json build failure",
        &output,
        1,
    )
    .expect("project-path `ql run --json` should exit with code 1 on build failure");
    expect_empty_stderr(
        "project-run-project-json-build-failure",
        "project run json build failure",
        &stderr,
    )
    .expect("project-path `ql run --json` build failure should stay on stdout");

    let json = parse_json_output("project-run-project-json-build-failure", &stdout);
    assert_eq!(json["schema"], "ql.run.v1");
    assert_eq!(
        json["path"],
        project_root.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["scope"], "project");
    assert_eq!(
        json["project_manifest_path"],
        app_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["requested_profile"], "debug");
    assert_eq!(json["profile_overridden"], false);
    assert_eq!(json["program_args"], serde_json::json!([]));
    assert_eq!(json["status"], "failed");
    assert_eq!(json["built_target"], JsonValue::Null);
    assert_eq!(json["execution"], JsonValue::Null);
    assert_eq!(json["failure"]["kind"], "build");
    assert_eq!(
        json["failure"]["build_failure"]["manifest_path"],
        app_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["failure"]["build_failure"]["package_name"], "app");
    assert_eq!(json["failure"]["build_failure"]["selected"], true);
    assert_eq!(json["failure"]["build_failure"]["dependency_only"], false);
    assert_eq!(json["failure"]["build_failure"]["kind"], "bin");
    assert_eq!(json["failure"]["build_failure"]["path"], "src/main.ql");
    assert_eq!(
        json["failure"]["build_failure"]["error_kind"],
        "diagnostics"
    );
    assert_eq!(
        json["failure"]["build_failure"]["message"],
        "build produced diagnostics"
    );
    assert_eq!(
        json["failure"]["build_failure"]["diagnostic_file"]["path"],
        main_path.display().to_string().replace('\\', "/")
    );
}

#[test]
fn run_package_path_uses_manifest_default_release_profile() {
    if !toolchain_available("`ql run` manifest profile test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-manifest-profile");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create package source tree for run manifest profile test");
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"

[profile]
default = "release"
"#,
    );
    temp.write("app/src/main.ql", "fn main() -> Int { return 13 }\n");
    let output_path = executable_output_path(&project_root.join("target/ql/release"), "main");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["run"]).arg(&project_root);
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
        &output_path,
        "manifest default profile executable",
        "manifest default profile run",
    )
    .expect("manifest default profile run should emit the release executable");
}

#[test]
fn run_workspace_path_uses_workspace_default_profile() {
    if !toolchain_available("`ql run` workspace profile test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-workspace-profile");
    let project_root = temp.path().join("workspace");
    std::fs::create_dir_all(project_root.join("packages/app/src"))
        .expect("create workspace package source tree for workspace profile run test");
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
    temp.write(
        "workspace/packages/app/src/main.ql",
        "fn main() -> Int { return 13 }\n",
    );
    let output_path =
        executable_output_path(&project_root.join("packages/app/target/ql/release"), "main");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["run"]).arg(&project_root);
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
        &output_path,
        "workspace default profile executable",
        "workspace default profile run",
    )
    .expect("workspace default profile run should emit the release executable");
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

#[test]
fn run_workspace_path_executes_the_only_runnable_target() {
    if !toolchain_available("`ql run` workspace test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-workspace");
    let project_root = temp.path().join("workspace");
    std::fs::create_dir_all(project_root.join("packages/app/src"))
        .expect("create app package source tree");
    std::fs::create_dir_all(project_root.join("packages/tool/src"))
        .expect("create tool package source tree");
    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/tool"]
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
        "workspace/packages/tool/qlang.toml",
        r#"
[package]
name = "tool"
"#,
    );
    temp.write(
        "workspace/packages/app/src/main.ql",
        "fn main() -> Int { return 11 }\n",
    );
    temp.write(
        "workspace/packages/tool/src/lib.ql",
        "pub fn helper() -> Int { return 2 }\n",
    );
    let output_path =
        executable_output_path(&project_root.join("packages/app/target/ql/debug"), "main");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["run"]).arg(&project_root);
    let output = run_command_capture(&mut command, "`ql run` workspace path");
    let (stdout, stderr) =
        expect_exit_code("project-run-workspace", "workspace path run", &output, 11)
            .expect("workspace-path `ql run` should exit with the runnable member status");
    expect_silent_output(
        "project-run-workspace",
        "workspace path run",
        &stdout,
        &stderr,
    )
    .expect("workspace-path `ql run` should leave stdout/stderr to the program");
    expect_file_exists(
        "project-run-workspace",
        &output_path,
        "workspace executable",
        "workspace path run",
    )
    .expect("workspace-path `ql run` should leave the built executable in the member target dir");
}

#[test]
fn run_project_source_file_list_uses_workspace_context_and_only_reports_runnable_targets() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-list-workspace-source");
    let project_root = temp.path().join("workspace");
    std::fs::create_dir_all(project_root.join("packages/app/src"))
        .expect("create app source tree for run list test");
    std::fs::create_dir_all(project_root.join("packages/tool/src"))
        .expect("create tool source tree for run list test");
    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/tool"]
"#,
    );
    let app_manifest = temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    let app_main = temp.write(
        "workspace/packages/app/src/main.ql",
        "fn main() -> Int { return 1 }\n",
    );
    temp.write(
        "workspace/packages/app/src/lib.ql",
        "pub fn helper() -> Int { return 2 }\n",
    );
    let tool_manifest = temp.write(
        "workspace/packages/tool/qlang.toml",
        r#"
[package]
name = "tool"
"#,
    );
    temp.write(
        "workspace/packages/tool/src/main.ql",
        "fn main() -> Int { return 3 }\n",
    );
    temp.write(
        "workspace/packages/tool/src/lib.ql",
        "pub fn helper() -> Int { return 4 }\n",
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["run"])
        .arg(&app_main)
        .args(["--list", "--json"]);
    let output = run_command_capture(
        &mut command,
        "`ql run --list --json` workspace member source path",
    );
    let (stdout, stderr) = expect_success(
        "project-run-list-workspace-source",
        "workspace member source runnable target listing",
        &output,
    )
    .expect("workspace member source `ql run --list --json` should succeed");
    expect_empty_stderr(
        "project-run-list-workspace-source",
        "workspace member source runnable target listing",
        &stderr,
    )
    .expect("workspace member source `ql run --list --json` should not print stderr");

    let json = parse_json_output("project-run-list-workspace-source", &stdout);
    let expected = serde_json::json!({
        "schema": "ql.project.targets.v1",
        "members": [
            {
                "manifest_path": app_manifest.display().to_string().replace('\\', "/"),
                "package_name": "app",
                "targets": [
                    {
                        "kind": "bin",
                        "path": "src/main.ql",
                    }
                ],
            },
            {
                "manifest_path": tool_manifest.display().to_string().replace('\\', "/"),
                "package_name": "tool",
                "targets": [
                    {
                        "kind": "bin",
                        "path": "src/main.ql",
                    }
                ],
            }
        ],
    });
    assert_eq!(
        json, expected,
        "workspace member source `ql run --list --json` should resolve the outer workspace and only report runnable targets"
    );
    assert!(
        !project_root.join("packages/app/target").exists(),
        "`ql run --list --json` should not build the selected source"
    );
}

#[test]
fn run_project_member_directory_list_uses_workspace_context_and_only_reports_runnable_targets() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-list-workspace-member-dir");
    let project_root = temp.path().join("workspace");
    let app_root = project_root.join("packages").join("app");
    let tool_root = project_root.join("packages").join("tool");
    std::fs::create_dir_all(app_root.join("src"))
        .expect("create app source tree for run list workspace member directory test");
    std::fs::create_dir_all(tool_root.join("src"))
        .expect("create tool source tree for run list workspace member directory test");
    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/tool"]
"#,
    );
    let app_manifest = temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace/packages/app/src/main.ql",
        "fn main() -> Int { return 1 }\n",
    );
    temp.write(
        "workspace/packages/app/src/lib.ql",
        "pub fn helper() -> Int { return 2 }\n",
    );
    let tool_manifest = temp.write(
        "workspace/packages/tool/qlang.toml",
        r#"
[package]
name = "tool"
"#,
    );
    temp.write(
        "workspace/packages/tool/src/main.ql",
        "fn main() -> Int { return 3 }\n",
    );
    temp.write(
        "workspace/packages/tool/src/lib.ql",
        "pub fn helper() -> Int { return 4 }\n",
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["run"])
        .arg(&app_root)
        .args(["--list", "--json"]);
    let output = run_command_capture(
        &mut command,
        "`ql run --list --json` workspace member directory",
    );
    let (stdout, stderr) = expect_success(
        "project-run-list-workspace-member-dir",
        "workspace member directory runnable target listing",
        &output,
    )
    .expect("workspace member directory `ql run --list --json` should succeed");
    expect_empty_stderr(
        "project-run-list-workspace-member-dir",
        "workspace member directory runnable target listing",
        &stderr,
    )
    .expect("workspace member directory `ql run --list --json` should not print stderr");

    let json = parse_json_output("project-run-list-workspace-member-dir", &stdout);
    let expected = serde_json::json!({
        "schema": "ql.project.targets.v1",
        "members": [
            {
                "manifest_path": app_manifest.display().to_string().replace('\\', "/"),
                "package_name": "app",
                "targets": [
                    {
                        "kind": "bin",
                        "path": "src/main.ql",
                    }
                ],
            },
            {
                "manifest_path": tool_manifest.display().to_string().replace('\\', "/"),
                "package_name": "tool",
                "targets": [
                    {
                        "kind": "bin",
                        "path": "src/main.ql",
                    }
                ],
            }
        ],
    });
    assert_eq!(
        json, expected,
        "workspace member directory `ql run --list --json` should resolve the outer workspace and only report runnable targets"
    );
    assert!(
        !project_root.join("packages/app/target").exists(),
        "`ql run --list --json` workspace member directory should not build the selected directory"
    );
}

#[test]
fn run_preserves_large_exit_code() {
    if !toolchain_available("`ql run` large-exit-code test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-large-exit");
    let source_path = temp.write("large_exit.ql", "fn main() -> Int { return 690 }\n");
    let output_path = executable_output_path(&temp.path().join("target/ql/debug"), "large_exit");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["run"]).arg(&source_path);
    let output = run_command_capture(&mut command, "`ql run` large exit code");
    let (stdout, stderr) = expect_exit_code(
        "project-run-large-exit",
        "large-exit-code run",
        &output,
        690,
    )
    .expect("`ql run` should preserve the child exit code");
    expect_silent_output(
        "project-run-large-exit",
        "large-exit-code run",
        &stdout,
        &stderr,
    )
    .expect("large-exit-code `ql run` should leave stdout/stderr to the program");
    expect_file_exists(
        "project-run-large-exit",
        &output_path,
        "large-exit executable",
        "large-exit-code run",
    )
    .expect("large-exit-code `ql run` should still leave the built executable in place");
}

#[test]
fn run_project_path_rejects_multiple_runnable_targets() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-multiple");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src/bin"))
        .expect("create package source tree for multi-target run test");
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write("app/src/main.ql", "fn main() -> Int { return 1 }\n");
    temp.write("app/src/bin/admin.ql", "fn main() -> Int { return 2 }\n");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["run"]).arg(&project_root);
    let output = run_command_capture(&mut command, "`ql run` multiple runnable targets");
    let (stdout, stderr) = expect_exit_code(
        "project-run-multiple",
        "multiple runnable target rejection",
        &output,
        1,
    )
    .expect("`ql run` should reject project paths with multiple runnable targets");
    expect_empty_stdout(
        "project-run-multiple",
        "multiple runnable target rejection",
        &stdout,
    )
    .expect("multiple runnable target rejection should not print stdout");
    expect_stderr_contains(
        "project-run-multiple",
        "multiple runnable target rejection",
        &stderr,
        "error: `ql run` found multiple runnable build targets",
    )
    .expect("multiple runnable target rejection should explain the ambiguity");
    expect_stderr_contains(
        "project-run-multiple",
        "multiple runnable target rejection",
        &stderr,
        "hint: rerun `ql run <source-file>`",
    )
    .expect("multiple runnable target rejection should point to a direct target rerun");
}

#[test]
fn run_project_path_selects_requested_binary_target() {
    if !toolchain_available("`ql run --bin` package test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-select-bin");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src/bin"))
        .expect("create package source tree for target selector run test");
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write("app/src/main.ql", "fn main() -> Int { return 1 }\n");
    temp.write("app/src/bin/admin.ql", "fn main() -> Int { return 2 }\n");
    let output_path = executable_output_path(&project_root.join("target/ql/debug/bin"), "admin");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["run"])
        .arg(&project_root)
        .args(["--bin", "admin"]);
    let output = run_command_capture(&mut command, "`ql run --bin` package path");
    let (stdout, stderr) = expect_exit_code(
        "project-run-select-bin",
        "selected binary target run",
        &output,
        2,
    )
    .expect("package-path `ql run --bin` should exit with the selected binary status");
    expect_silent_output(
        "project-run-select-bin",
        "selected binary target run",
        &stdout,
        &stderr,
    )
    .expect("package-path `ql run --bin` should leave stdout/stderr to the program");
    expect_file_exists(
        "project-run-select-bin",
        &output_path,
        "selected binary executable",
        "selected binary target run",
    )
    .expect(
        "package-path `ql run --bin` should build the selected executable in the bin target dir",
    );
}

#[test]
fn run_library_only_package_reports_no_runnable_targets() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-library-only");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create package source tree for no-runnable-target test");
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write("app/src/lib.ql", "pub fn helper() -> Int { return 1 }\n");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["run"]).arg(&project_root);
    let output = run_command_capture(&mut command, "`ql run` library-only package");
    let (stdout, stderr) = expect_exit_code(
        "project-run-library-only",
        "library-only run rejection",
        &output,
        1,
    )
    .expect("`ql run` should reject packages without runnable targets");
    expect_empty_stdout(
        "project-run-library-only",
        "library-only run rejection",
        &stdout,
    )
    .expect("library-only run rejection should not print stdout");
    expect_stderr_contains(
        "project-run-library-only",
        "library-only run rejection",
        &stderr,
        "error: `ql run` found no runnable build targets",
    )
    .expect("library-only run rejection should explain the missing runnable target");
    expect_stderr_contains(
        "project-run-library-only",
        "library-only run rejection",
        &stderr,
        "hint: add `src/main.ql`, `src/bin/*.ql`, or declare `[[bin]].path`",
    )
    .expect("library-only run rejection should explain how to make the package runnable");
}

#[test]
fn run_package_path_syncs_dependency_interfaces_without_polluting_program_output() {
    if !toolchain_available("`ql run` dependency sync test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-dependency-sync");
    let dep_root = temp.path().join("dep");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(dep_root.join("src")).expect("create dependency source tree");
    std::fs::create_dir_all(project_root.join("src")).expect("create package source tree");
    temp.write(
        "dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "dep/src/lib.ql",
        "extern \"c\" pub fn q_add(left: Int, right: Int) -> Int { return left + right }\n",
    );
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"

[dependencies]
dep = "../dep"
"#,
    );
    temp.write(
        "app/src/main.ql",
        "use dep.q_add as add\n\nfn main() -> Int { return add(6, 7) }\n",
    );

    let interface_output = dep_root.join("dep.qi");
    let dependency_output = static_library_output_path(&dep_root.join("target/ql/debug"), "lib");
    let executable_output = executable_output_path(&project_root.join("target/ql/debug"), "main");
    assert!(
        !interface_output.exists(),
        "dependency interface should start missing for sync test"
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["run"]).arg(&project_root);
    let output = run_command_capture(&mut command, "`ql run` dependency sync");
    let (stdout, stderr) = expect_exit_code(
        "project-run-dependency-sync",
        "package path run with dependency sync",
        &output,
        13,
    )
    .expect("package-path `ql run` should sync dependency interfaces before execution");
    expect_silent_output(
        "project-run-dependency-sync",
        "package path run with dependency sync",
        &stdout,
        &stderr,
    )
    .expect("dependency-sync run should leave stdout/stderr to the program");
    expect_file_exists(
        "project-run-dependency-sync",
        &interface_output,
        "synced dependency interface",
        "package path run with dependency sync",
    )
    .expect("dependency-sync run should emit the dependency interface");
    expect_file_exists(
        "project-run-dependency-sync",
        &dependency_output,
        "dependency package artifact",
        "package path run with dependency sync",
    )
    .expect("dependency-sync run should also build the dependency package artifact");
    expect_file_exists(
        "project-run-dependency-sync",
        &executable_output,
        "package executable",
        "package path run with dependency sync",
    )
    .expect("dependency-sync run should still emit the executable artifact");
}

#[test]
fn run_package_path_supports_direct_dependency_public_functions() {
    if !toolchain_available("`ql run` dependency public function test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-dependency-public-function");
    let dep_root = temp.path().join("dep");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(dep_root.join("src")).expect("create dependency source tree");
    std::fs::create_dir_all(project_root.join("src")).expect("create package source tree");
    temp.write(
        "dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "dep/src/lib.ql",
        "pub fn add(left: Int, right: Int) -> Int { return left + right }\n",
    );
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"

[dependencies]
dep = "../dep"
"#,
    );
    temp.write(
        "app/src/main.ql",
        "use dep.add as sum\n\nfn main() -> Int { return sum(9, 4) }\n",
    );

    let interface_output = dep_root.join("dep.qi");
    let dependency_output = static_library_output_path(&dep_root.join("target/ql/debug"), "lib");
    let executable_output = executable_output_path(&project_root.join("target/ql/debug"), "main");
    assert!(
        !interface_output.exists(),
        "dependency interface should start missing for direct dependency public function test"
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["run"]).arg(&project_root);
    let output = run_command_capture(&mut command, "`ql run` dependency public function");
    let (stdout, stderr) = expect_exit_code(
        "project-run-dependency-public-function",
        "package path run with dependency public function",
        &output,
        13,
    )
    .expect("package-path `ql run` should support direct dependency public functions");
    expect_silent_output(
        "project-run-dependency-public-function",
        "package path run with dependency public function",
        &stdout,
        &stderr,
    )
    .expect("dependency public function run should leave stdout/stderr to the program");
    expect_file_exists(
        "project-run-dependency-public-function",
        &interface_output,
        "synced dependency interface",
        "package path run with dependency public function",
    )
    .expect("dependency public function run should emit the dependency interface");
    expect_file_exists(
        "project-run-dependency-public-function",
        &dependency_output,
        "dependency package artifact",
        "package path run with dependency public function",
    )
    .expect("dependency public function run should also build the dependency package artifact");
    expect_file_exists(
        "project-run-dependency-public-function",
        &executable_output,
        "package executable",
        "package path run with dependency public function",
    )
    .expect("dependency public function run should still emit the executable artifact");
}

#[test]
fn run_package_path_supports_direct_dependency_public_values() {
    if !toolchain_available("`ql run` dependency public value test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-dependency-public-value");
    let dep_root = temp.path().join("dep");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(dep_root.join("src")).expect("create dependency source tree");
    std::fs::create_dir_all(project_root.join("src")).expect("create package source tree");
    temp.write(
        "dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "dep/src/lib.ql",
        "pub const VALUE: Int = 7\npub static READY: Bool = true\npub static VALUES: [Int; 3] = [1, 3, 5]\n",
    );
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"

[dependencies]
dep = "../dep"
"#,
    );
    temp.write(
        "app/src/main.ql",
        "use dep.VALUE as THRESHOLD\nuse dep.READY as ENABLED\nuse dep.VALUES as ITEMS\n\nfn main() -> Int {\n    if ENABLED {\n        return THRESHOLD + ITEMS[1]\n    }\n    return 0\n}\n",
    );

    let interface_output = dep_root.join("dep.qi");
    let dependency_output = static_library_output_path(&dep_root.join("target/ql/debug"), "lib");
    let executable_output = executable_output_path(&project_root.join("target/ql/debug"), "main");
    assert!(
        !interface_output.exists(),
        "dependency interface should start missing for direct dependency public value test"
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["run"]).arg(&project_root);
    let output = run_command_capture(&mut command, "`ql run` dependency public value");
    let (stdout, stderr) = expect_exit_code(
        "project-run-dependency-public-value",
        "package path run with dependency public value",
        &output,
        10,
    )
    .expect("package-path `ql run` should support direct dependency public values");
    expect_silent_output(
        "project-run-dependency-public-value",
        "package path run with dependency public value",
        &stdout,
        &stderr,
    )
    .expect("dependency public value run should leave stdout/stderr to the program");
    expect_file_exists(
        "project-run-dependency-public-value",
        &interface_output,
        "synced dependency interface",
        "package path run with dependency public value",
    )
    .expect("dependency public value run should emit the dependency interface");
    expect_file_exists(
        "project-run-dependency-public-value",
        &dependency_output,
        "dependency package artifact",
        "package path run with dependency public value",
    )
    .expect("dependency public value run should also build the dependency package artifact");
    expect_file_exists(
        "project-run-dependency-public-value",
        &executable_output,
        "package executable",
        "package path run with dependency public value",
    )
    .expect("dependency public value run should still emit the executable artifact");
}

#[test]
fn run_package_path_supports_dependency_public_values_with_function_initializers() {
    if !toolchain_available("`ql run` dependency public value initializer test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-dependency-public-value-function-initializers");
    let dep_root = temp.path().join("dep");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(dep_root.join("src")).expect("create dependency source tree");
    std::fs::create_dir_all(project_root.join("src")).expect("create package source tree");
    temp.write(
        "dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "dep/src/lib.ql",
        "pub fn add_one(value: Int) -> Int { return value + 1 }\npub fn make_value() -> Int { return add_one(6) }\npub const VALUE: Int = make_value()\npub const APPLY: (Int) -> Int = add_one\n",
    );
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"

[dependencies]
dep = "../dep"
"#,
    );
    temp.write(
        "app/src/main.ql",
        "use dep.VALUE as VALUE_ALIAS\nuse dep.APPLY as RUN\n\nfn main() -> Int { return VALUE_ALIAS + RUN(3) }\n",
    );

    let interface_output = dep_root.join("dep.qi");
    let dependency_output = static_library_output_path(&dep_root.join("target/ql/debug"), "lib");
    let executable_output = executable_output_path(&project_root.join("target/ql/debug"), "main");
    assert!(
        !interface_output.exists(),
        "dependency interface should start missing for dependency public value initializer test"
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["run"]).arg(&project_root);
    let output = run_command_capture(&mut command, "`ql run` dependency public value initializer");
    let (stdout, stderr) = expect_exit_code(
        "project-run-dependency-public-value-function-initializers",
        "package path run with dependency public value function initializers",
        &output,
        11,
    )
    .expect(
        "package-path `ql run` should support dependency public values with function initializers",
    );
    expect_silent_output(
        "project-run-dependency-public-value-function-initializers",
        "package path run with dependency public value function initializers",
        &stdout,
        &stderr,
    )
    .expect("dependency public value initializer run should leave stdout/stderr to the program");
    expect_file_exists(
        "project-run-dependency-public-value-function-initializers",
        &interface_output,
        "synced dependency interface",
        "package path run with dependency public value function initializers",
    )
    .expect("dependency public value initializer run should emit the dependency interface");
    expect_file_exists(
        "project-run-dependency-public-value-function-initializers",
        &dependency_output,
        "dependency package artifact",
        "package path run with dependency public value function initializers",
    )
    .expect(
        "dependency public value initializer run should also build the dependency package artifact",
    );
    expect_file_exists(
        "project-run-dependency-public-value-function-initializers",
        &executable_output,
        "package executable",
        "package path run with dependency public value function initializers",
    )
    .expect("dependency public value initializer run should still emit the executable artifact");
}

#[test]
fn run_package_path_supports_direct_dependency_public_struct_functions() {
    if !toolchain_available("`ql run` dependency public struct function test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-dependency-public-struct-function");
    let dep_root = temp.path().join("dep");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(dep_root.join("src")).expect("create dependency source tree");
    std::fs::create_dir_all(project_root.join("src")).expect("create package source tree");
    temp.write(
        "dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "dep/src/lib.ql",
        "pub struct Box { value: Int }\npub fn make_box() -> Box { return Box { value: 7 } }\n",
    );
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"

[dependencies]
dep = "../dep"
"#,
    );
    temp.write(
        "app/src/main.ql",
        "use dep.make_box as make\n\nfn main() -> Int {\n    let value = make()\n    return value.value\n}\n",
    );

    let interface_output = dep_root.join("dep.qi");
    let dependency_output = static_library_output_path(&dep_root.join("target/ql/debug"), "lib");
    let executable_output = executable_output_path(&project_root.join("target/ql/debug"), "main");
    assert!(
        !interface_output.exists(),
        "dependency interface should start missing for direct dependency public struct function test"
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["run"]).arg(&project_root);
    let output = run_command_capture(&mut command, "`ql run` dependency public struct function");
    let (stdout, stderr) = expect_exit_code(
        "project-run-dependency-public-struct-function",
        "package path run with dependency public struct function",
        &output,
        7,
    )
    .expect("package-path `ql run` should support direct dependency public struct functions");
    expect_silent_output(
        "project-run-dependency-public-struct-function",
        "package path run with dependency public struct function",
        &stdout,
        &stderr,
    )
    .expect("dependency public struct function run should leave stdout/stderr to the program");
    expect_file_exists(
        "project-run-dependency-public-struct-function",
        &interface_output,
        "synced dependency interface",
        "package path run with dependency public struct function",
    )
    .expect("dependency public struct function run should emit the dependency interface");
    expect_file_exists(
        "project-run-dependency-public-struct-function",
        &dependency_output,
        "dependency package artifact",
        "package path run with dependency public struct function",
    )
    .expect(
        "dependency public struct function run should also build the dependency package artifact",
    );
    expect_file_exists(
        "project-run-dependency-public-struct-function",
        &executable_output,
        "package executable",
        "package path run with dependency public struct function",
    )
    .expect("dependency public struct function run should still emit the executable artifact");
}

#[test]
fn run_package_path_supports_direct_dependency_public_struct_methods() {
    if !toolchain_available("`ql run` dependency public struct method test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-dependency-public-struct-method");
    let dep_root = temp.path().join("dep");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(dep_root.join("src")).expect("create dependency source tree");
    std::fs::create_dir_all(project_root.join("src")).expect("create package source tree");
    temp.write(
        "dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "dep/src/lib.ql",
        "pub struct Box { value: Int }\n\nimpl Box {\n    pub fn read(self) -> Int {\n        return self.value\n    }\n}\n\npub fn make_box() -> Box { return Box { value: 7 } }\n",
    );
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"

[dependencies]
dep = "../dep"
"#,
    );
    temp.write(
        "app/src/main.ql",
        "use dep.make_box as make\n\nfn main() -> Int {\n    let value = make()\n    return value.read()\n}\n",
    );

    let interface_output = dep_root.join("dep.qi");
    let dependency_output = static_library_output_path(&dep_root.join("target/ql/debug"), "lib");
    let executable_output = executable_output_path(&project_root.join("target/ql/debug"), "main");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["run"]).arg(&project_root);
    let output = run_command_capture(&mut command, "`ql run` dependency public struct method");
    let (stdout, stderr) = expect_exit_code(
        "project-run-dependency-public-struct-method",
        "package path run with dependency public struct method",
        &output,
        7,
    )
    .expect("package-path `ql run` should support direct dependency public struct methods");
    expect_silent_output(
        "project-run-dependency-public-struct-method",
        "package path run with dependency public struct method",
        &stdout,
        &stderr,
    )
    .expect("dependency public struct method run should leave stdout/stderr to the program");
    expect_file_exists(
        "project-run-dependency-public-struct-method",
        &interface_output,
        "synced dependency interface",
        "package path run with dependency public struct method",
    )
    .expect("dependency public struct method run should emit the dependency interface");
    expect_file_exists(
        "project-run-dependency-public-struct-method",
        &dependency_output,
        "dependency package artifact",
        "package path run with dependency public struct method",
    )
    .expect(
        "dependency public struct method run should also build the dependency package artifact",
    );
    expect_file_exists(
        "project-run-dependency-public-struct-method",
        &executable_output,
        "package executable",
        "package path run with dependency public struct method",
    )
    .expect("dependency public struct method run should still emit the executable artifact");
}
