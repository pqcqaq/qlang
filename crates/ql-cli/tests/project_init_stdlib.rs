mod support;

use std::path::Path;

use serde_json::Value as JsonValue;
use support::project_init_stdlib::{
    assert_repo_stdlib_run_json, assert_repo_stdlib_starter_build_json,
    assert_repo_stdlib_starter_test_json, assert_stdlib_check_json,
    assert_stdlib_dependencies_json, assert_stdlib_graph_json,
    assert_stdlib_status_member_dependencies, assert_stdlib_status_member_targets,
    expect_emit_interface_check_ok, expect_stdlib_starter_interface,
    expect_stdlib_starter_main_source, expect_stdlib_starter_smoke_source,
    expect_stdlib_starter_source, json_path, parse_json_output, toolchain_available,
    write_repo_stdlib_fixture,
};
use support::{
    TempDir, executable_output_path, expect_empty_stderr, expect_exit_code, expect_file_exists,
    expect_silent_output, expect_stdout_contains_all, expect_success, ql_command,
    read_normalized_file, run_command_capture, static_library_output_path, workspace_root,
};

fn assert_stdlib_dependency_build_targets(context: &str, build_json: &JsonValue) {
    let built_targets = build_json["built_targets"]
        .as_array()
        .unwrap_or_else(|| panic!("{context} should expose built targets: {build_json}"));
    for package_name in [
        "std.array",
        "std.core",
        "std.option",
        "std.result",
        "std.test",
    ] {
        assert!(
            built_targets.iter().any(|target| {
                target["package_name"] == package_name
                    && target["dependency_only"] == true
                    && target["kind"] == "lib"
                    && target["selected"] == false
            }),
            "{context} should include dependency target `{package_name}`: {build_json}"
        );
    }
}

fn assert_build_json_includes_target(context: &str, build_json: &JsonValue, expected: JsonValue) {
    let built_targets = build_json["built_targets"]
        .as_array()
        .unwrap_or_else(|| panic!("{context} should expose built targets: {build_json}"));
    assert!(
        built_targets.iter().any(|target| target == &expected),
        "{context} should include target {expected}: {build_json}"
    );
}

fn assert_stdlib_workspace_graph_json(
    context: &str,
    graph_json: &JsonValue,
    member_manifest: &Path,
) {
    assert_stdlib_graph_json(
        context,
        graph_json,
        "app",
        member_manifest,
        "app.qi",
        "../../../stdlib/packages",
    );
}

fn assert_stdlib_workspace_status_json(
    context: &str,
    status_json: &JsonValue,
    project_root: &Path,
    member_root: &Path,
) {
    assert_eq!(status_json["schema"], "ql.project.status.v1");
    assert_eq!(status_json["path"], json_path(project_root));
    assert_eq!(
        status_json["project_manifest_path"],
        json_path(&project_root.join("qlang.toml"))
    );
    assert_eq!(status_json["kind"], "workspace");
    assert_eq!(status_json["status"], "ok");
    let members = status_json["members"]
        .as_array()
        .unwrap_or_else(|| panic!("{context} should expose members: {status_json}"));
    assert_eq!(members.len(), 1, "{context} should select only app member");
    let member = &members[0];
    assert_eq!(member["member"], "packages/app");
    assert_eq!(member["package_name"], "app");
    assert_eq!(
        member["manifest_path"],
        json_path(&member_root.join("qlang.toml"))
    );
    assert_eq!(
        member["interface"]["path"],
        json_path(&member_root.join("app.qi"))
    );
    assert_eq!(member["interface"]["status"], "valid");
    assert_eq!(member["interface"]["detail"], JsonValue::Null);
    assert_eq!(member["interface"]["stale_reasons"], serde_json::json!([]));

    assert_stdlib_status_member_targets(context, member);
    assert_stdlib_status_member_dependencies(context, member, "../../../stdlib/packages");
}

#[test]
fn repo_stdlib_fixture_builds_runs_and_tests_starter_package() {
    if !toolchain_available("`ql build/run/test --package` copied repo stdlib starter") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-repo-stdlib-workspace-starter-package");
    let stdlib_root = write_repo_stdlib_fixture(&temp, &workspace_root);

    let mut build = ql_command(&workspace_root);
    build
        .args(["build"])
        .arg(&stdlib_root)
        .args(["--package", "stdlib.starter", "--json"]);
    let output = run_command_capture(
        &mut build,
        "`ql build --package stdlib.starter --json` copied repo stdlib",
    );
    let (stdout, stderr) = expect_success(
        "repo-stdlib-workspace-starter-package-fixture",
        "build copied repo stdlib starter package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "repo-stdlib-workspace-starter-package-fixture",
        "build copied repo stdlib starter package",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("repo-stdlib-workspace-starter-package-fixture", &stdout);
    assert_repo_stdlib_starter_build_json(
        "copied repo stdlib starter build json",
        &actual,
        &stdlib_root,
    );

    for (context, path) in [
        (
            "copied stdlib starter static library",
            static_library_output_path(
                &stdlib_root.join("examples/starter/target/ql/debug"),
                "lib",
            ),
        ),
        (
            "copied stdlib starter llvm-ir",
            stdlib_root.join("examples/starter/target/ql/debug/main.ll"),
        ),
    ] {
        expect_file_exists(
            "repo-stdlib-workspace-starter-package-fixture",
            &path,
            context,
            "`ql build --package stdlib.starter --json` copied repo stdlib",
        )
        .unwrap();
    }

    let mut run = ql_command(&workspace_root);
    run.args(["run"])
        .arg(&stdlib_root)
        .args(["--package", "stdlib.starter", "--json"]);
    let output = run_command_capture(
        &mut run,
        "`ql run --package stdlib.starter --json` copied repo stdlib",
    );
    let (stdout, stderr) = expect_success(
        "repo-stdlib-workspace-starter-package-fixture",
        "run copied repo stdlib starter package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "repo-stdlib-workspace-starter-package-fixture",
        "run copied repo stdlib starter package",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("repo-stdlib-workspace-starter-package-fixture", &stdout);
    assert_repo_stdlib_run_json("copied repo stdlib starter run json", &actual, &stdlib_root);
    expect_file_exists(
        "repo-stdlib-workspace-starter-package-fixture",
        &executable_output_path(
            &stdlib_root.join("examples/starter/target/ql/debug"),
            "main",
        ),
        "copied stdlib starter executable",
        "`ql run --package stdlib.starter --json` copied repo stdlib",
    )
    .unwrap();

    let mut test = ql_command(&workspace_root);
    test.args(["test"])
        .arg(&stdlib_root)
        .args(["--package", "stdlib.starter", "--json"]);
    let output = run_command_capture(
        &mut test,
        "`ql test --package stdlib.starter --json` copied repo stdlib",
    );
    let (stdout, stderr) = expect_success(
        "repo-stdlib-workspace-starter-package-fixture",
        "test copied repo stdlib starter package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "repo-stdlib-workspace-starter-package-fixture",
        "test copied repo stdlib starter package",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("repo-stdlib-workspace-starter-package-fixture", &stdout);
    assert_repo_stdlib_starter_test_json(
        "copied repo stdlib starter test json",
        &actual,
        &stdlib_root,
    );
    expect_file_exists(
        "repo-stdlib-workspace-starter-package-fixture",
        &executable_output_path(
            &stdlib_root.join("examples/starter/target/ql/debug/tests"),
            "smoke",
        ),
        "copied stdlib starter smoke executable",
        "`ql test --package stdlib.starter --json` copied repo stdlib",
    )
    .unwrap();
}

#[test]
fn project_init_with_stdlib_creates_runnable_and_testable_package_scaffold() {
    if !toolchain_available("`ql project init --stdlib` runnable package test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-init-stdlib-package-run");
    let stdlib_root = write_repo_stdlib_fixture(&temp, &workspace_root);
    let project_root = temp.path().join("demo-package");
    let package_manifest = project_root.join("qlang.toml");
    let package_library_output =
        static_library_output_path(&project_root.join("target/ql/debug"), "lib");
    let package_build_output = project_root.join("target/ql/debug/main.ll");
    let package_run_output = executable_output_path(&project_root.join("target/ql/debug"), "main");
    let package_interface_output = project_root.join("demo-package.qi");

    let mut init = ql_command(&workspace_root);
    init.args([
        "project",
        "init",
        &project_root.to_string_lossy(),
        "--stdlib",
        &stdlib_root.to_string_lossy(),
    ]);
    let output = run_command_capture(&mut init, "`ql project init --stdlib` runnable package");
    let (_stdout, stderr) = expect_success(
        "project-init-stdlib-package-run",
        "stdlib package init for runnable scaffold",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-package-run",
        "stdlib package init for runnable scaffold",
        &stderr,
    )
    .unwrap();

    let mut build_json = ql_command(&workspace_root);
    build_json.current_dir(temp.path());
    build_json.args(["build"]).arg(&project_root).arg("--json");
    let output = run_command_capture(
        &mut build_json,
        "`ql build --json` initialized stdlib package",
    );
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-package-run",
        "json build initialized stdlib package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-package-run",
        "json build initialized stdlib package",
        &stderr,
    )
    .unwrap();

    let build_json = parse_json_output("project-init-stdlib-package-run", &stdout);
    assert_eq!(build_json["schema"], "ql.build.v1");
    assert_eq!(
        build_json["path"],
        project_root.display().to_string().replace('\\', "/")
    );
    assert_eq!(build_json["scope"], "project");
    assert_eq!(
        build_json["project_manifest_path"],
        package_manifest.display().to_string().replace('\\', "/")
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
                "manifest_path": package_manifest.display().to_string().replace('\\', "/"),
                "package_name": "demo-package",
                "selected": true,
                "status": "wrote",
                "path": package_interface_output.display().to_string().replace('\\', "/"),
            }
        ])
    );
    assert_stdlib_dependency_build_targets("initialized stdlib package build json", &build_json);
    assert_build_json_includes_target(
        "initialized stdlib package build json",
        &build_json,
        serde_json::json!({
            "manifest_path": package_manifest.display().to_string().replace('\\', "/"),
            "package_name": "demo-package",
            "selected": true,
            "dependency_only": false,
            "kind": "lib",
            "path": "src/lib.ql",
            "emit": "staticlib",
            "profile": "debug",
            "artifact_path": package_library_output.display().to_string().replace('\\', "/"),
            "c_header_path": JsonValue::Null,
        }),
    );
    assert_build_json_includes_target(
        "initialized stdlib package build json",
        &build_json,
        serde_json::json!({
            "manifest_path": package_manifest.display().to_string().replace('\\', "/"),
            "package_name": "demo-package",
            "selected": true,
            "dependency_only": false,
            "kind": "bin",
            "path": "src/main.ql",
            "emit": "llvm-ir",
            "profile": "debug",
            "artifact_path": package_build_output.display().to_string().replace('\\', "/"),
            "c_header_path": JsonValue::Null,
        }),
    );
    expect_file_exists(
        "project-init-stdlib-package-run",
        &package_library_output,
        "initialized stdlib package library artifact",
        "json build initialized stdlib package",
    )
    .unwrap();
    expect_file_exists(
        "project-init-stdlib-package-run",
        &package_build_output,
        "initialized stdlib package build artifact",
        "json build initialized stdlib package",
    )
    .unwrap();
    expect_file_exists(
        "project-init-stdlib-package-run",
        &package_interface_output,
        "initialized stdlib package interface artifact",
        "json build initialized stdlib package",
    )
    .unwrap();

    let mut run = ql_command(&workspace_root);
    run.current_dir(temp.path());
    run.args(["run"]).arg(&project_root);
    let output = run_command_capture(&mut run, "`ql run` initialized stdlib package");
    let (stdout, stderr) = expect_exit_code(
        "project-init-stdlib-package-run",
        "run initialized stdlib package",
        &output,
        0,
    )
    .unwrap();
    expect_silent_output(
        "project-init-stdlib-package-run",
        "run initialized stdlib package",
        &stdout,
        &stderr,
    )
    .unwrap();

    let mut run_json = ql_command(&workspace_root);
    run_json.current_dir(temp.path());
    run_json.args(["run"]).arg(&project_root).arg("--json");
    let output = run_command_capture(&mut run_json, "`ql run --json` initialized stdlib package");
    let (stdout, stderr) = expect_exit_code(
        "project-init-stdlib-package-run",
        "json run initialized stdlib package",
        &output,
        0,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-package-run",
        "json run initialized stdlib package",
        &stderr,
    )
    .unwrap();

    let run_json = parse_json_output("project-init-stdlib-package-run", &stdout);
    assert_eq!(run_json["schema"], "ql.run.v1");
    assert_eq!(
        run_json["path"],
        project_root.display().to_string().replace('\\', "/")
    );
    assert_eq!(run_json["scope"], "project");
    assert_eq!(
        run_json["project_manifest_path"],
        package_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(run_json["requested_profile"], "debug");
    assert_eq!(run_json["profile_overridden"], false);
    assert_eq!(run_json["program_args"], serde_json::json!([]));
    assert_eq!(run_json["status"], "completed");
    assert_eq!(run_json["failure"], JsonValue::Null);
    assert_eq!(
        run_json["built_target"],
        serde_json::json!({
            "manifest_path": package_manifest.display().to_string().replace('\\', "/"),
            "package_name": "demo-package",
            "selected": true,
            "dependency_only": false,
            "kind": "bin",
            "path": "src/main.ql",
            "emit": "exe",
            "profile": "debug",
            "artifact_path": package_run_output.display().to_string().replace('\\', "/"),
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
        "project-init-stdlib-package-run",
        &package_run_output,
        "initialized stdlib package executable",
        "json run initialized stdlib package",
    )
    .unwrap();

    let mut test = ql_command(&workspace_root);
    test.current_dir(temp.path());
    test.args(["test"]).arg(&project_root);
    let output = run_command_capture(&mut test, "`ql test` initialized stdlib package");
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-package-run",
        "test initialized stdlib package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-package-run",
        "test initialized stdlib package",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "project-init-stdlib-package-run",
        &stdout.replace('\\', "/"),
        &[
            "test tests/smoke.ql ... ok",
            "test result: ok. 1 passed; 0 failed",
        ],
    )
    .unwrap();

    let mut test_json = ql_command(&workspace_root);
    test_json.current_dir(temp.path());
    test_json.args(["test", "--json"]).arg(&project_root);
    let output = run_command_capture(
        &mut test_json,
        "`ql test --json` initialized stdlib package",
    );
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-package-run",
        "json test initialized stdlib package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-package-run",
        "json test initialized stdlib package",
        &stderr,
    )
    .unwrap();

    let actual = parse_json_output("project-init-stdlib-package-run", &stdout);
    let expected = serde_json::json!({
        "schema": "ql.test.v1",
        "path": project_root.display().to_string().replace('\\', "/"),
        "requested_profile": "debug",
        "profile_overridden": false,
        "package_name": JsonValue::Null,
        "filter": JsonValue::Null,
        "list_only": false,
        "status": "ok",
        "discovered_total": 1,
        "selected_total": 1,
        "targets": [
            {
                "path": "tests/smoke.ql",
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
        "initialized stdlib package should keep a stable test json contract"
    );
}

#[test]
fn project_init_with_stdlib_creates_consuming_workspace_scaffold_and_check_succeeds() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-init-stdlib-workspace");
    let stdlib_root = write_repo_stdlib_fixture(&temp, &workspace_root);
    let project_root = temp.path().join("demo-workspace");
    let member_root = project_root.join("packages/app");

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
    let output = run_command_capture(&mut init, "`ql project init --workspace --stdlib`");
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-workspace",
        "stdlib workspace init",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-workspace",
        "stdlib workspace init",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "project-init-stdlib-workspace",
        &stdout,
        &[
            &format!(
                "created: {}",
                project_root
                    .join("qlang.toml")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            &format!(
                "created: {}",
                member_root
                    .join("qlang.toml")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            &format!(
                "created: {}",
                member_root
                    .join("tests/smoke.ql")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
        ],
    )
    .unwrap();

    assert_eq!(
        read_normalized_file(
            &project_root.join("qlang.toml"),
            "stdlib workspace manifest"
        ),
        "[workspace]\nmembers = [\"packages/app\"]\n"
    );
    assert_eq!(
        read_normalized_file(
            &member_root.join("qlang.toml"),
            "stdlib workspace member manifest"
        ),
        "[package]\nname = \"app\"\n\n[dependencies]\n\"std.core\" = \"../../../stdlib/packages/core\"\n\"std.option\" = \"../../../stdlib/packages/option\"\n\"std.result\" = \"../../../stdlib/packages/result\"\n\"std.array\" = \"../../../stdlib/packages/array\"\n\"std.test\" = \"../../../stdlib/packages/test\"\n"
    );
    let lib_source = read_normalized_file(
        &member_root.join("src/lib.ql"),
        "stdlib workspace member source",
    );
    expect_stdlib_starter_source(&lib_source, "stdlib workspace member source");
    let main_source = read_normalized_file(
        &member_root.join("src/main.ql"),
        "stdlib workspace member main source",
    );
    expect_stdlib_starter_main_source(&main_source, "stdlib workspace member main source");
    let smoke_source = read_normalized_file(
        &member_root.join("tests/smoke.ql"),
        "stdlib workspace member smoke test",
    );
    expect_stdlib_starter_smoke_source(&smoke_source, "stdlib workspace member smoke test");

    let mut check = ql_command(&workspace_root);
    check.args(["check", &project_root.to_string_lossy()]);
    let output = run_command_capture(&mut check, "`ql check` initialized stdlib workspace");
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-workspace",
        "check initialized stdlib workspace",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-workspace",
        "check initialized stdlib workspace",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "project-init-stdlib-workspace",
        &stdout.replace('\\', "/"),
        &[
            &format!(
                "ok: {}",
                member_root
                    .join("src/lib.ql")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            "loaded interface:",
        ],
    )
    .unwrap();

    let mut check_json = ql_command(&workspace_root);
    check_json.args(["check", &project_root.to_string_lossy(), "--json"]);
    let output = run_command_capture(
        &mut check_json,
        "`ql check --json` initialized stdlib workspace",
    );
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-workspace",
        "json check initialized stdlib workspace",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-workspace",
        "json check initialized stdlib workspace",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("project-init-stdlib-workspace", &stdout);
    assert_stdlib_check_json(
        "initialized stdlib workspace check json",
        &actual,
        "workspace",
        &project_root.join("qlang.toml"),
        &[
            member_root.join("src/lib.ql"),
            member_root.join("src/main.ql"),
        ],
        &stdlib_root,
    );

    let member_interface = member_root.join("app.qi");
    let mut emit_interface = ql_command(&workspace_root);
    emit_interface.args([
        "project",
        "emit-interface",
        &project_root.to_string_lossy(),
        "--package",
        "app",
    ]);
    let output = run_command_capture(
        &mut emit_interface,
        "`ql project emit-interface --package app` initialized stdlib workspace",
    );
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-workspace",
        "emit interface initialized stdlib workspace package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-workspace",
        "emit interface initialized stdlib workspace package",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "project-init-stdlib-workspace",
        &stdout.replace('\\', "/"),
        &[&format!(
            "wrote interface: {}",
            member_interface.display().to_string().replace('\\', "/")
        )],
    )
    .unwrap();
    let interface_source = read_normalized_file(
        &member_interface,
        "initialized stdlib workspace member interface artifact",
    );
    expect_stdlib_starter_interface(
        &interface_source,
        "app",
        "initialized stdlib workspace member interface artifact",
    );
    expect_emit_interface_check_ok(
        "project-init-stdlib-workspace",
        &workspace_root,
        &project_root,
        Some("app"),
        &member_interface,
        "`ql project emit-interface --check --package app` initialized stdlib workspace",
    );

    let mut graph_json = ql_command(&workspace_root);
    graph_json.args([
        "project",
        "graph",
        &project_root.to_string_lossy(),
        "--package",
        "app",
        "--json",
    ]);
    let output = run_command_capture(
        &mut graph_json,
        "`ql project graph --json --package app` initialized stdlib workspace",
    );
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-workspace",
        "graph json initialized stdlib workspace package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-workspace",
        "graph json initialized stdlib workspace package",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("project-init-stdlib-workspace", &stdout);
    assert_stdlib_workspace_graph_json(
        "initialized stdlib workspace graph json",
        &actual,
        &member_root.join("qlang.toml"),
    );

    let mut status_json = ql_command(&workspace_root);
    status_json.args([
        "project",
        "status",
        &project_root.to_string_lossy(),
        "--package",
        "app",
        "--json",
    ]);
    let output = run_command_capture(
        &mut status_json,
        "`ql project status --json --package app` initialized stdlib workspace",
    );
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-workspace",
        "status json initialized stdlib workspace package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-workspace",
        "status json initialized stdlib workspace package",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("project-init-stdlib-workspace", &stdout);
    assert_stdlib_workspace_status_json(
        "initialized stdlib workspace status json",
        &actual,
        &project_root,
        &member_root,
    );

    let mut dependencies = ql_command(&workspace_root);
    dependencies.args([
        "project",
        "dependencies",
        &project_root.to_string_lossy(),
        "--name",
        "app",
    ]);
    let output = run_command_capture(
        &mut dependencies,
        "`ql project dependencies --name app` initialized stdlib workspace",
    );
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-workspace",
        "dependencies initialized stdlib workspace package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-workspace",
        "dependencies initialized stdlib workspace package",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "project-init-stdlib-workspace",
        &stdout.replace('\\', "/"),
        &[
            &format!(
                "workspace_manifest: {}",
                project_root
                    .join("qlang.toml")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            "package: app",
            "dependencies:",
            "  - ../../../stdlib/packages/core (std.core, local)",
            "  - ../../../stdlib/packages/option (std.option, local)",
            "  - ../../../stdlib/packages/result (std.result, local)",
            "  - ../../../stdlib/packages/array (std.array, local)",
            "  - ../../../stdlib/packages/test (std.test, local)",
        ],
    )
    .unwrap();

    let mut dependencies_json = ql_command(&workspace_root);
    dependencies_json.args([
        "project",
        "dependencies",
        &project_root.to_string_lossy(),
        "--name",
        "app",
        "--json",
    ]);
    let output = run_command_capture(
        &mut dependencies_json,
        "`ql project dependencies --json --name app` initialized stdlib workspace",
    );
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-workspace",
        "dependencies json initialized stdlib workspace package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-workspace",
        "dependencies json initialized stdlib workspace package",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("project-init-stdlib-workspace", &stdout);
    assert_stdlib_dependencies_json(
        "initialized stdlib workspace dependencies json",
        &actual,
        &project_root,
        &project_root.join("qlang.toml"),
        "app",
        "../../../stdlib/packages",
        &stdlib_root,
    );
}

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
