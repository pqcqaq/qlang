mod support;

use serde_json::Value as JsonValue;
use support::project_init_stdlib::{
    assert_build_json_includes_target, assert_stdlib_dependency_build_targets, parse_json_output,
    toolchain_available, write_repo_stdlib_fixture,
};
use support::{
    TempDir, executable_output_path, expect_empty_stderr, expect_exit_code, expect_file_exists,
    expect_silent_output, expect_stdout_contains_all, expect_success, ql_command,
    run_command_capture, static_library_output_path, workspace_root,
};

#[test]
fn project_init_with_stdlib_creates_runnable_and_testable_workspace_scaffold() {
    if !toolchain_available("`ql project init --workspace --stdlib` runnable workspace test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-init-stdlib-workspace-run");
    let stdlib_root = write_repo_stdlib_fixture(&temp, &workspace_root);
    let project_root = temp.path().join("demo-workspace");
    let member_root = project_root.join("packages/app");
    let app_manifest = member_root.join("qlang.toml");
    let app_library_output =
        static_library_output_path(&member_root.join("target/ql/debug"), "lib");
    let app_build_output = member_root.join("target/ql/debug/main.ll");
    let app_output = executable_output_path(&member_root.join("target/ql/debug"), "main");
    let app_interface_output = member_root.join("app.qi");

    let mut init = ql_command(&workspace_root);
    init.args([
        "project",
        "init",
        &project_root.to_string_lossy(),
        "--workspace",
        "--name",
        "app",
        "--stdlib",
        &stdlib_root.to_string_lossy(),
    ]);
    let output = run_command_capture(
        &mut init,
        "`ql project init --workspace --stdlib` runnable workspace",
    );
    let (_stdout, stderr) = expect_success(
        "project-init-stdlib-workspace-run",
        "stdlib workspace init for runnable scaffold",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-workspace-run",
        "stdlib workspace init for runnable scaffold",
        &stderr,
    )
    .unwrap();

    let mut build_json = ql_command(&workspace_root);
    build_json.current_dir(temp.path());
    build_json
        .args(["build"])
        .arg(&project_root)
        .args(["--package", "app", "--json"]);
    let output = run_command_capture(
        &mut build_json,
        "`ql build --json --package app` initialized stdlib workspace",
    );
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-workspace-run",
        "json build initialized stdlib workspace package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-workspace-run",
        "json build initialized stdlib workspace package",
        &stderr,
    )
    .unwrap();

    let build_json = parse_json_output("project-init-stdlib-workspace-run", &stdout);
    assert_eq!(build_json["schema"], "ql.build.v1");
    assert_eq!(
        build_json["path"],
        project_root.display().to_string().replace('\\', "/")
    );
    assert_eq!(build_json["scope"], "project");
    assert_eq!(
        build_json["project_manifest_path"],
        project_root
            .join("qlang.toml")
            .display()
            .to_string()
            .replace('\\', "/")
    );
    assert_eq!(build_json["requested_emit"], "llvm-ir");
    assert_eq!(build_json["requested_profile"], "debug");
    assert_eq!(build_json["profile_overridden"], false);
    assert_eq!(build_json["emit_interface"], false);
    assert_eq!(build_json["status"], "ok");
    assert_eq!(build_json["failure"], JsonValue::Null);
    assert_eq!(
        build_json["interfaces"],
        serde_json::json!([
            {
                "manifest_path": app_manifest.display().to_string().replace('\\', "/"),
                "package_name": "app",
                "selected": true,
                "status": "wrote",
                "path": app_interface_output.display().to_string().replace('\\', "/"),
            }
        ])
    );
    assert_stdlib_dependency_build_targets("initialized stdlib workspace build json", &build_json);
    assert_build_json_includes_target(
        "initialized stdlib workspace build json",
        &build_json,
        serde_json::json!({
            "manifest_path": app_manifest.display().to_string().replace('\\', "/"),
            "package_name": "app",
            "selected": true,
            "dependency_only": false,
            "kind": "lib",
            "path": "src/lib.ql",
            "emit": "staticlib",
            "profile": "debug",
            "artifact_path": app_library_output.display().to_string().replace('\\', "/"),
            "c_header_path": JsonValue::Null,
        }),
    );
    assert_build_json_includes_target(
        "initialized stdlib workspace build json",
        &build_json,
        serde_json::json!({
            "manifest_path": app_manifest.display().to_string().replace('\\', "/"),
            "package_name": "app",
            "selected": true,
            "dependency_only": false,
            "kind": "bin",
            "path": "src/main.ql",
            "emit": "llvm-ir",
            "profile": "debug",
            "artifact_path": app_build_output.display().to_string().replace('\\', "/"),
            "c_header_path": JsonValue::Null,
        }),
    );
    expect_file_exists(
        "project-init-stdlib-workspace-run",
        &app_library_output,
        "initialized stdlib workspace library artifact",
        "json build initialized stdlib workspace package",
    )
    .unwrap();
    expect_file_exists(
        "project-init-stdlib-workspace-run",
        &app_build_output,
        "initialized stdlib workspace build artifact",
        "json build initialized stdlib workspace package",
    )
    .unwrap();
    expect_file_exists(
        "project-init-stdlib-workspace-run",
        &app_interface_output,
        "initialized stdlib workspace interface artifact",
        "json build initialized stdlib workspace package",
    )
    .unwrap();

    let mut run = ql_command(&workspace_root);
    run.current_dir(temp.path());
    run.args(["run"]).arg(&project_root);
    let output = run_command_capture(&mut run, "`ql run` initialized stdlib workspace");
    let (stdout, stderr) = expect_exit_code(
        "project-init-stdlib-workspace-run",
        "run initialized stdlib workspace",
        &output,
        0,
    )
    .unwrap();
    expect_silent_output(
        "project-init-stdlib-workspace-run",
        "run initialized stdlib workspace",
        &stdout,
        &stderr,
    )
    .unwrap();

    let mut run_json = ql_command(&workspace_root);
    run_json.current_dir(temp.path());
    run_json
        .args(["run"])
        .arg(&project_root)
        .args(["--package", "app", "--json"]);
    let output = run_command_capture(
        &mut run_json,
        "`ql run --json --package app` initialized stdlib workspace",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-init-stdlib-workspace-run",
        "json run initialized stdlib workspace package",
        &output,
        0,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-workspace-run",
        "json run initialized stdlib workspace package",
        &stderr,
    )
    .unwrap();
    let run_json = parse_json_output("project-init-stdlib-workspace-run", &stdout);
    assert_eq!(run_json["schema"], "ql.run.v1");
    assert_eq!(
        run_json["path"],
        project_root.display().to_string().replace('\\', "/")
    );
    assert_eq!(run_json["scope"], "project");
    assert_eq!(
        run_json["project_manifest_path"],
        project_root
            .join("qlang.toml")
            .display()
            .to_string()
            .replace('\\', "/")
    );
    assert_eq!(run_json["requested_profile"], "debug");
    assert_eq!(run_json["profile_overridden"], false);
    assert_eq!(run_json["program_args"], serde_json::json!([]));
    assert_eq!(run_json["status"], "completed");
    assert_eq!(run_json["failure"], JsonValue::Null);
    assert_eq!(
        run_json["built_target"],
        serde_json::json!({
            "manifest_path": member_root.join("qlang.toml").display().to_string().replace('\\', "/"),
            "package_name": "app",
            "selected": true,
            "dependency_only": false,
            "kind": "bin",
            "path": "src/main.ql",
            "emit": "exe",
            "profile": "debug",
            "artifact_path": app_output.display().to_string().replace('\\', "/"),
            "c_header_path": JsonValue::Null,
        })
    );
    assert_eq!(
        run_json["execution"],
        serde_json::json!({
            "exit_code": 0,
            "stdout": "",
            "stderr": "",
        })
    );
    expect_file_exists(
        "project-init-stdlib-workspace-run",
        &app_output,
        "initialized stdlib workspace executable",
        "json run initialized stdlib workspace package",
    )
    .unwrap();

    let mut test = ql_command(&workspace_root);
    test.current_dir(temp.path());
    test.args(["test"]).arg(&project_root);
    let output = run_command_capture(&mut test, "`ql test` initialized stdlib workspace");
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-workspace-run",
        "test initialized stdlib workspace",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-workspace-run",
        "test initialized stdlib workspace",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "project-init-stdlib-workspace-run",
        &stdout.replace('\\', "/"),
        &[
            "test packages/app/tests/smoke.ql ... ok",
            "test result: ok. 1 passed; 0 failed",
        ],
    )
    .unwrap();

    let mut test_json = ql_command(&workspace_root);
    test_json.current_dir(temp.path());
    test_json
        .args(["test", "--json"])
        .arg(&project_root)
        .args(["--package", "app"]);
    let output = run_command_capture(
        &mut test_json,
        "`ql test --json --package app` initialized stdlib workspace",
    );
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-workspace-run",
        "json test initialized stdlib workspace package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-workspace-run",
        "json test initialized stdlib workspace package",
        &stderr,
    )
    .unwrap();

    let actual = parse_json_output("project-init-stdlib-workspace-run", &stdout);
    let expected = serde_json::json!({
        "schema": "ql.test.v1",
        "path": project_root.display().to_string().replace('\\', "/"),
        "requested_profile": "debug",
        "profile_overridden": false,
        "package_name": "app",
        "filter": JsonValue::Null,
        "list_only": false,
        "status": "ok",
        "discovered_total": 1,
        "selected_total": 1,
        "targets": [
            {
                "path": "packages/app/tests/smoke.ql",
                "kind": "smoke",
                "profile": "debug",
            }
        ],
        "passed": 1,
        "failed": 0,
        "failures": [],
    });
    assert_eq!(
        actual, expected,
        "initialized stdlib workspace should keep a stable package-selected test json contract"
    );
}
