mod support;

use serde_json::Value as JsonValue;
use support::{
    TempDir, expect_empty_stderr, expect_exit_code, expect_file_exists, expect_success, ql_command,
    run_command_capture, static_library_output_path, workspace_root,
};

fn normalize_output_text(text: &str) -> String {
    text.replace("\r\n", "\n")
}

fn parse_json_output(case_name: &str, stdout: &str) -> JsonValue {
    serde_json::from_str(&normalize_output_text(stdout))
        .unwrap_or_else(|error| panic!("[{case_name}] parse json stdout: {error}\n{stdout}"))
}

#[test]
fn build_package_path_supports_json_output_for_dependency_build_plan() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-package-json");
    let dep_root = temp.path().join("dep");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(dep_root.join("src")).expect("create dependency source tree");
    std::fs::create_dir_all(project_root.join("src")).expect("create package source tree");

    let dep_manifest = temp.write(
        "dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write("dep/src/lib.ql", "pub fn exported() -> Int { return 1 }\n");
    let app_manifest = temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"

[dependencies]
dep = "../dep"
"#,
    );
    temp.write("app/src/lib.ql", "pub fn helper() -> Int { return 1 }\n");
    temp.write("app/src/main.ql", "fn main() -> Int { return 0 }\n");

    let dep_output = static_library_output_path(&dep_root.join("target/ql/debug"), "lib");
    let app_lib_output = static_library_output_path(&project_root.join("target/ql/debug"), "lib");
    let app_main_output = project_root.join("target/ql/debug/main.ll");
    let interface_output = project_root.join("app.qi");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["build"])
        .arg(&project_root)
        .args(["--emit-interface", "--json"]);
    let output = run_command_capture(&mut command, "`ql build --json` package dependency plan");
    let (stdout, stderr) = expect_success(
        "project-build-package-json",
        "package dependency-plan build json",
        &output,
    )
    .expect("package-path `ql build --json` should succeed");
    expect_empty_stderr(
        "project-build-package-json",
        "package dependency-plan build json",
        &stderr,
    )
    .expect("package-path `ql build --json` should not print stderr");

    let json = parse_json_output("project-build-package-json", &stdout);
    let expected = serde_json::json!({
        "schema": "ql.build.v1",
        "path": project_root.display().to_string().replace('\\', "/"),
        "scope": "project",
        "project_manifest_path": app_manifest.display().to_string().replace('\\', "/"),
        "requested_emit": "llvm-ir",
        "requested_profile": "debug",
        "profile_overridden": false,
        "emit_interface": true,
        "status": "ok",
        "failure": JsonValue::Null,
        "built_targets": [
            {
                "manifest_path": dep_manifest.display().to_string().replace('\\', "/"),
                "package_name": "dep",
                "selected": false,
                "dependency_only": true,
                "kind": "lib",
                "path": "src/lib.ql",
                "emit": "staticlib",
                "profile": "debug",
                "artifact_path": dep_output.display().to_string().replace('\\', "/"),
                "c_header_path": JsonValue::Null,
            },
            {
                "manifest_path": app_manifest.display().to_string().replace('\\', "/"),
                "package_name": "app",
                "selected": true,
                "dependency_only": false,
                "kind": "lib",
                "path": "src/lib.ql",
                "emit": "staticlib",
                "profile": "debug",
                "artifact_path": app_lib_output.display().to_string().replace('\\', "/"),
                "c_header_path": JsonValue::Null,
            },
            {
                "manifest_path": app_manifest.display().to_string().replace('\\', "/"),
                "package_name": "app",
                "selected": true,
                "dependency_only": false,
                "kind": "bin",
                "path": "src/main.ql",
                "emit": "llvm-ir",
                "profile": "debug",
                "artifact_path": app_main_output.display().to_string().replace('\\', "/"),
                "c_header_path": JsonValue::Null,
            }
        ],
        "interfaces": [
            {
                "manifest_path": app_manifest.display().to_string().replace('\\', "/"),
                "package_name": "app",
                "selected": true,
                "status": "wrote",
                "path": interface_output.display().to_string().replace('\\', "/"),
            }
        ],
    });
    assert_eq!(
        json, expected,
        "package-path `ql build --json` should match the stable contract"
    );

    expect_file_exists(
        "project-build-package-json",
        &dep_output,
        "dependency package artifact",
        "package dependency-plan build json",
    )
    .expect("package-path `ql build --json` should emit the dependency artifact");
    expect_file_exists(
        "project-build-package-json",
        &app_lib_output,
        "package library artifact",
        "package dependency-plan build json",
    )
    .expect("package-path `ql build --json` should emit the package library artifact");
    expect_file_exists(
        "project-build-package-json",
        &app_main_output,
        "package binary artifact",
        "package dependency-plan build json",
    )
    .expect("package-path `ql build --json` should emit the package binary artifact");
    expect_file_exists(
        "project-build-package-json",
        &interface_output,
        "package interface artifact",
        "package dependency-plan build json",
    )
    .expect("package-path `ql build --json` should emit the package interface");
}

#[test]
fn build_package_path_json_reports_dependency_interface_prep_manifest_failure() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-package-json-dependency-prep-manifest");
    let dep_root = temp.path().join("dep");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(&dep_root).expect("create dependency root for prep manifest failure");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create package source tree for prep manifest failure");

    let dep_manifest = temp.write(
        "dep/qlang.toml",
        r#"
[package]
"#,
    );
    let app_manifest = temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"

[dependencies]
dep = "../dep"
"#,
    );
    temp.write("app/src/main.ql", "fn main() -> Int { return 0 }\n");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["build"]).arg(&project_root).arg("--json");
    let output = run_command_capture(
        &mut command,
        "`ql build --json` dependency interface prep manifest failure",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-build-package-json-dependency-prep-manifest",
        "package build json dependency interface prep manifest failure",
        &output,
        1,
    )
    .expect("package-path `ql build --json` should fail on dependency prep manifest failures");
    expect_empty_stderr(
        "project-build-package-json-dependency-prep-manifest",
        "package build json dependency interface prep manifest failure",
        &stderr,
    )
    .expect("dependency prep manifest failures should stay on stdout in json mode");

    let json = parse_json_output(
        "project-build-package-json-dependency-prep-manifest",
        &stdout,
    );
    assert_eq!(json["schema"], "ql.build.v1");
    assert_eq!(
        json["path"],
        project_root.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["scope"], "project");
    assert_eq!(
        json["project_manifest_path"],
        app_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["status"], "failed");
    assert_eq!(json["built_targets"], serde_json::json!([]));
    assert_eq!(json["interfaces"], serde_json::json!([]));
    assert_eq!(
        json["failure"]["manifest_path"],
        dep_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["failure"]["package_name"], JsonValue::Null);
    assert_eq!(json["failure"]["selected"], false);
    assert_eq!(json["failure"]["dependency_only"], true);
    assert_eq!(json["failure"]["kind"], "interface");
    assert_eq!(
        json["failure"]["path"],
        dep_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["failure"]["error_kind"], "manifest");
    assert_eq!(json["failure"]["stage"], "dependency-interface-prep");
    assert_eq!(
        json["failure"]["owner_manifest_path"],
        app_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(
        json["failure"]["reference_manifest_path"],
        dep_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["failure"]["reference"], "../dep");
    assert_eq!(json["failure"]["failing_dependency_count"], 1);
    assert_eq!(
        json["failure"]["first_failing_dependency_manifest"],
        dep_manifest.display().to_string().replace('\\', "/")
    );
    assert!(
        json["failure"]["message"]
            .as_str()
            .expect("dependency prep manifest failure should expose a message")
            .contains("does not declare `[package].name`"),
        "dependency prep manifest failure should preserve the broken dependency manifest detail: {json}"
    );
}

#[test]
fn build_package_path_json_reports_dependency_interface_prep_output_failure() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-package-json-dependency-prep-output");
    let dep_root = temp.path().join("dep");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(dep_root.join("src"))
        .expect("create dependency source tree for prep output failure");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create package source tree for prep output failure");

    let dep_manifest = temp.write(
        "dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write("dep/src/lib.ql", "pub fn exported() -> Int { return 1 }\n");
    let interface_output = dep_root.join("dep.qi");
    std::fs::create_dir_all(&interface_output)
        .expect("create blocking dependency interface directory for prep output failure");
    let app_manifest = temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"

[dependencies]
dep = "../dep"
"#,
    );
    temp.write("app/src/main.ql", "fn main() -> Int { return 0 }\n");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["build"]).arg(&project_root).arg("--json");
    let output = run_command_capture(
        &mut command,
        "`ql build --json` dependency interface prep output failure",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-build-package-json-dependency-prep-output",
        "package build json dependency interface prep output failure",
        &output,
        1,
    )
    .expect("package-path `ql build --json` should fail on dependency prep output failures");
    expect_empty_stderr(
        "project-build-package-json-dependency-prep-output",
        "package build json dependency interface prep output failure",
        &stderr,
    )
    .expect("dependency prep output failures should stay on stdout in json mode");

    let json = parse_json_output("project-build-package-json-dependency-prep-output", &stdout);
    assert_eq!(json["schema"], "ql.build.v1");
    assert_eq!(
        json["path"],
        project_root.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["scope"], "project");
    assert_eq!(
        json["project_manifest_path"],
        app_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["status"], "failed");
    assert_eq!(json["built_targets"], serde_json::json!([]));
    assert_eq!(json["interfaces"], serde_json::json!([]));
    assert_eq!(
        json["failure"]["manifest_path"],
        dep_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["failure"]["package_name"], JsonValue::Null);
    assert_eq!(json["failure"]["selected"], false);
    assert_eq!(json["failure"]["dependency_only"], true);
    assert_eq!(json["failure"]["kind"], "interface");
    assert_eq!(
        json["failure"]["path"],
        interface_output.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["failure"]["error_kind"], "interface-output");
    assert_eq!(json["failure"]["stage"], "dependency-interface-prep");
    assert_eq!(
        json["failure"]["output_path"],
        interface_output.display().to_string().replace('\\', "/")
    );
    assert_eq!(
        json["failure"]["owner_manifest_path"],
        app_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(
        json["failure"]["reference_manifest_path"],
        dep_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["failure"]["reference"], "../dep");
    assert_eq!(json["failure"]["failing_dependency_count"], 1);
    assert_eq!(
        json["failure"]["first_failing_dependency_manifest"],
        dep_manifest.display().to_string().replace('\\', "/")
    );
    assert!(
        json["failure"]["message"]
            .as_str()
            .expect("dependency prep output failure should expose a message")
            .contains("failed to write interface"),
        "dependency prep output failure should preserve the blocked interface write detail: {json}"
    );
    assert!(
        interface_output.is_dir(),
        "dependency prep output failure should preserve `{}` as a directory",
        interface_output.display()
    );
}

#[test]
fn build_package_path_json_reports_build_plan_dependency_cycle() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-package-json-build-plan-cycle");
    let app_root = temp.path().join("app");
    let core_root = temp.path().join("core");
    std::fs::create_dir_all(app_root.join("src"))
        .expect("create app source tree for build plan cycle test");
    std::fs::create_dir_all(core_root.join("src"))
        .expect("create core source tree for build plan cycle test");

    let app_manifest = temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"

[dependencies]
core = "../core"
"#,
    );
    let core_manifest = temp.write(
        "core/qlang.toml",
        r#"
[package]
name = "core"

[dependencies]
app = "../app"
"#,
    );
    temp.write("app/src/main.ql", "fn main() -> Int { return 0 }\n");
    temp.write("core/src/lib.ql", "pub fn answer() -> Int { return 42 }\n");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["build"]).arg(&app_root).arg("--json");
    let output = run_command_capture(
        &mut command,
        "`ql build --json` package build-plan dependency cycle",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-build-package-json-build-plan-cycle",
        "package build json build-plan dependency cycle failure",
        &output,
        1,
    )
    .expect("package-path `ql build --json` should fail on build-plan dependency cycles");
    expect_empty_stderr(
        "project-build-package-json-build-plan-cycle",
        "package build json build-plan dependency cycle failure",
        &stderr,
    )
    .expect("build-plan dependency cycle failures should stay on stdout in json mode");

    let json = parse_json_output("project-build-package-json-build-plan-cycle", &stdout);
    assert_eq!(json["schema"], "ql.build.v1");
    assert_eq!(
        json["path"],
        app_root.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["scope"], "project");
    assert_eq!(
        json["project_manifest_path"],
        app_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["status"], "failed");
    assert_eq!(json["built_targets"], serde_json::json!([]));
    assert_eq!(json["interfaces"], serde_json::json!([]));
    assert_eq!(
        json["failure"]["manifest_path"],
        app_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["failure"]["package_name"], JsonValue::Null);
    assert_eq!(json["failure"]["selected"], JsonValue::Null);
    assert_eq!(json["failure"]["dependency_only"], JsonValue::Null);
    assert_eq!(json["failure"]["kind"], JsonValue::Null);
    assert_eq!(
        json["failure"]["path"],
        app_root.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["failure"]["error_kind"], "cycle");
    assert_eq!(json["failure"]["stage"], "build-plan");
    assert_eq!(
        json["failure"]["message"],
        "local package build dependencies contain a cycle"
    );
    assert_eq!(json["failure"]["owner_manifest_path"], JsonValue::Null);
    assert_eq!(
        json["failure"]["dependency_manifest_path"],
        app_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(
        json["failure"]["cycle_manifests"],
        serde_json::json!([
            app_manifest.display().to_string().replace('\\', "/"),
            core_manifest.display().to_string().replace('\\', "/"),
            app_manifest.display().to_string().replace('\\', "/"),
        ])
    );
}

#[test]
fn build_package_path_reports_build_plan_dependency_cycle_without_stack_overflow() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-package-build-plan-cycle");
    let app_root = temp.path().join("app");
    let core_root = temp.path().join("core");
    std::fs::create_dir_all(app_root.join("src"))
        .expect("create app source tree for non-json build plan cycle test");
    std::fs::create_dir_all(core_root.join("src"))
        .expect("create core source tree for non-json build plan cycle test");

    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"

[dependencies]
core = "../core"
"#,
    );
    temp.write(
        "core/qlang.toml",
        r#"
[package]
name = "core"

[dependencies]
app = "../app"
"#,
    );
    temp.write("app/src/main.ql", "fn main() -> Int { return 0 }\n");
    temp.write("core/src/lib.ql", "pub fn answer() -> Int { return 42 }\n");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["build"]).arg(&app_root);
    let output = run_command_capture(
        &mut command,
        "`ql build` package build-plan dependency cycle",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-build-package-build-plan-cycle",
        "package build build-plan dependency cycle failure",
        &output,
        1,
    )
    .expect("package-path `ql build` should fail on build-plan dependency cycles");
    assert!(
        !stdout.contains("has overflowed its stack"),
        "non-json build-plan dependency cycle should not crash, got stdout:\n{stdout}"
    );
    assert!(
        stderr.contains("error: `ql build` local package build dependencies contain a cycle"),
        "non-json build-plan dependency cycle should report the cycle instead of crashing, got:\n{stderr}"
    );
    assert!(
        stderr.contains("note: cycle manifests:"),
        "non-json build-plan dependency cycle should print cycle manifest detail, got:\n{stderr}"
    );
}

#[test]
fn build_package_path_json_reports_build_plan_dependency_target_discovery_failure() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-package-json-build-plan-target-discovery");
    let dep_root = temp.path().join("dep");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(dep_root.join("src"))
        .expect("create dependency source tree for build plan target discovery test");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create package source tree for build plan target discovery test");

    let dep_manifest = temp.write(
        "dep/qlang.toml",
        r#"
[package]
name = "dep"

[lib]
path = "src/missing.ql"
"#,
    );
    temp.write("dep/src/api.ql", "pub fn exported() -> Int { return 1 }\n");
    let app_manifest = temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"

[dependencies]
dep = "../dep"
"#,
    );
    temp.write("app/src/main.ql", "fn main() -> Int { return 0 }\n");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["build"]).arg(&project_root).arg("--json");
    let output = run_command_capture(
        &mut command,
        "`ql build --json` dependency target discovery failure",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-build-package-json-build-plan-target-discovery",
        "package build json build-plan dependency target discovery failure",
        &output,
        1,
    )
    .expect("package-path `ql build --json` should fail on build-plan dependency target discovery failures");
    expect_empty_stderr(
        "project-build-package-json-build-plan-target-discovery",
        "package build json build-plan dependency target discovery failure",
        &stderr,
    )
    .expect("build-plan dependency target discovery failures should stay on stdout in json mode");

    let json = parse_json_output(
        "project-build-package-json-build-plan-target-discovery",
        &stdout,
    );
    assert_eq!(json["schema"], "ql.build.v1");
    assert_eq!(
        json["path"],
        project_root.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["scope"], "project");
    assert_eq!(
        json["project_manifest_path"],
        app_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["status"], "failed");
    assert_eq!(json["built_targets"], serde_json::json!([]));
    assert_eq!(json["interfaces"], serde_json::json!([]));
    assert_eq!(
        json["failure"]["manifest_path"],
        dep_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["failure"]["package_name"], JsonValue::Null);
    assert_eq!(json["failure"]["selected"], JsonValue::Null);
    assert_eq!(json["failure"]["dependency_only"], JsonValue::Null);
    assert_eq!(json["failure"]["kind"], JsonValue::Null);
    assert_eq!(
        json["failure"]["path"],
        project_root.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["failure"]["error_kind"], "dependency");
    assert_eq!(json["failure"]["stage"], "build-plan");
    assert_eq!(
        json["failure"]["owner_manifest_path"],
        app_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(
        json["failure"]["dependency_manifest_path"],
        dep_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["failure"]["cycle_manifests"], JsonValue::Null);
    assert!(
        json["failure"]["message"]
            .as_str()
            .expect("build-plan dependency failure should expose a message")
            .contains("`[lib].path` declares missing target"),
        "build-plan dependency failure should preserve the dependency target discovery error: {json}"
    );
}

#[test]
fn build_package_path_json_reports_emit_interface_source_failure() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-package-json-emit-interface-source-failure");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create package source tree for emit-interface source failure test");

    let app_manifest = temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write("app/src/lib.ql", "pub fn helper() -> Int { return 1 }\n");
    let extra_source = temp.write(
        "app/src/extra.ql",
        "fn broken() -> Int { return \"oops\" }\n",
    );

    let app_lib_output = static_library_output_path(&project_root.join("target/ql/debug"), "lib");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["build"])
        .arg(&project_root)
        .args(["--emit-interface", "--json"]);
    let output = run_command_capture(
        &mut command,
        "`ql build --emit-interface --json` package interface source failure",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-build-package-json-emit-interface-source-failure",
        "package interface source failure build json",
        &output,
        1,
    )
    .expect("package-path `ql build --emit-interface --json` should exit with code 1");
    expect_empty_stderr(
        "project-build-package-json-emit-interface-source-failure",
        "package interface source failure build json",
        &stderr,
    )
    .expect("package-path `ql build --emit-interface --json` should not print stderr");

    let json = parse_json_output(
        "project-build-package-json-emit-interface-source-failure",
        &stdout,
    );
    assert_eq!(json["schema"], "ql.build.v1");
    assert_eq!(
        json["path"],
        project_root.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["scope"], "project");
    assert_eq!(
        json["project_manifest_path"],
        app_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["status"], "failed");
    assert_eq!(json["emit_interface"], true);
    assert_eq!(json["interfaces"], serde_json::json!([]));
    assert_eq!(json["built_targets"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        json["failure"]["manifest_path"],
        app_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["failure"]["package_name"], "app");
    assert_eq!(json["failure"]["selected"], true);
    assert_eq!(json["failure"]["dependency_only"], false);
    assert_eq!(json["failure"]["kind"], "interface");
    assert_eq!(
        json["failure"]["path"],
        app_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["failure"]["error_kind"], "package-sources");
    assert_eq!(json["failure"]["stage"], "emit-interface");
    assert_eq!(
        json["failure"]["message"],
        "package interface emission found 1 failing source file(s)"
    );
    assert_eq!(json["failure"]["output_path"], JsonValue::Null);
    assert_eq!(json["failure"]["source_root"], JsonValue::Null);
    assert_eq!(json["failure"]["failing_source_count"], 1);
    assert_eq!(
        json["failure"]["first_failing_source"],
        extra_source.display().to_string().replace('\\', "/")
    );

    expect_file_exists(
        "project-build-package-json-emit-interface-source-failure",
        &app_lib_output,
        "package library artifact",
        "package interface source failure build json",
    )
    .expect("package-path `ql build --emit-interface --json` should preserve the build artifact");
}

#[test]
fn build_package_path_json_reports_emit_interface_output_failure() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-package-json-emit-interface-output-failure");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create package source tree for emit-interface output failure test");

    let app_manifest = temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write("app/src/lib.ql", "pub fn helper() -> Int { return 1 }\n");
    let interface_output = project_root.join("app.qi");
    std::fs::create_dir_all(&interface_output)
        .expect("occupy the default interface output path with a directory");
    let app_lib_output = static_library_output_path(&project_root.join("target/ql/debug"), "lib");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["build"])
        .arg(&project_root)
        .args(["--emit-interface", "--json"]);
    let output = run_command_capture(
        &mut command,
        "`ql build --emit-interface --json` package interface output failure",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-build-package-json-emit-interface-output-failure",
        "package interface output failure build json",
        &output,
        1,
    )
    .expect("package-path `ql build --emit-interface --json` should exit with code 1");
    expect_empty_stderr(
        "project-build-package-json-emit-interface-output-failure",
        "package interface output failure build json",
        &stderr,
    )
    .expect("package-path `ql build --emit-interface --json` should not print stderr");

    let json = parse_json_output(
        "project-build-package-json-emit-interface-output-failure",
        &stdout,
    );
    assert_eq!(json["schema"], "ql.build.v1");
    assert_eq!(
        json["path"],
        project_root.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["scope"], "project");
    assert_eq!(
        json["project_manifest_path"],
        app_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["status"], "failed");
    assert_eq!(json["emit_interface"], true);
    assert_eq!(json["interfaces"], serde_json::json!([]));
    assert_eq!(json["built_targets"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        json["failure"]["manifest_path"],
        app_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["failure"]["package_name"], "app");
    assert_eq!(json["failure"]["selected"], true);
    assert_eq!(json["failure"]["dependency_only"], false);
    assert_eq!(json["failure"]["kind"], "interface");
    assert_eq!(
        json["failure"]["path"],
        interface_output.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["failure"]["error_kind"], "interface-output");
    assert_eq!(json["failure"]["stage"], "emit-interface");
    assert!(
        json["failure"]["message"]
            .as_str()
            .expect("interface output failure json should expose a message")
            .contains(&format!(
                "failed to write interface `{}`",
                interface_output.display().to_string().replace('\\', "/")
            )),
        "interface output failure json should preserve the write failure context: {json}"
    );
    assert_eq!(
        json["failure"]["output_path"],
        interface_output.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["failure"]["source_root"], JsonValue::Null);
    assert_eq!(json["failure"]["failing_source_count"], JsonValue::Null);
    assert_eq!(json["failure"]["first_failing_source"], JsonValue::Null);

    expect_file_exists(
        "project-build-package-json-emit-interface-output-failure",
        &app_lib_output,
        "package library artifact",
        "package interface output failure build json",
    )
    .expect("package-path `ql build --emit-interface --json` should preserve the build artifact");
}

#[test]
fn build_project_json_reports_invalid_manifest_failure() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-json-invalid-manifest");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(&project_root)
        .expect("create package root for invalid manifest json test");
    let manifest_path = temp.write(
        "app/qlang.toml",
        r#"
[package
name = "app"
"#,
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["build"]).arg(&project_root).arg("--json");
    let output = run_command_capture(&mut command, "`ql build --json` invalid manifest");
    let (stdout, stderr) = expect_exit_code(
        "project-build-json-invalid-manifest",
        "invalid manifest build json failure",
        &output,
        1,
    )
    .expect("project-path `ql build --json` invalid manifest should exit with code 1");
    expect_empty_stderr(
        "project-build-json-invalid-manifest",
        "invalid manifest build json failure",
        &stderr,
    )
    .expect("project-path `ql build --json` invalid manifest should not print stderr");

    let json = parse_json_output("project-build-json-invalid-manifest", &stdout);
    assert_eq!(json["schema"], "ql.build.v1");
    assert_eq!(
        json["path"],
        project_root.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["scope"], "project");
    assert_eq!(json["project_manifest_path"], JsonValue::Null);
    assert_eq!(json["status"], "failed");
    assert_eq!(json["built_targets"], serde_json::json!([]));
    assert_eq!(json["interfaces"], serde_json::json!([]));
    assert_eq!(
        json["failure"]["manifest_path"],
        manifest_path.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["failure"]["package_name"], JsonValue::Null);
    assert_eq!(json["failure"]["selected"], JsonValue::Null);
    assert_eq!(json["failure"]["dependency_only"], JsonValue::Null);
    assert_eq!(json["failure"]["kind"], JsonValue::Null);
    assert_eq!(
        json["failure"]["path"],
        project_root.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["failure"]["error_kind"], "manifest");
    assert_eq!(json["failure"]["stage"], "manifest-load");
    assert!(
        json["failure"]["message"]
            .as_str()
            .expect("invalid manifest json failure should expose a message")
            .contains("invalid manifest"),
        "invalid manifest json failure should preserve the manifest parse failure: {json}"
    );
}

#[test]
fn build_project_json_reports_selector_miss_failure() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-json-selector-miss");
    let project_root = temp.path().join("workspace");
    std::fs::create_dir_all(project_root.join("packages/app/src"))
        .expect("create app package source tree");
    std::fs::create_dir_all(project_root.join("packages/tool/src"))
        .expect("create tool package source tree");

    let workspace_manifest = temp.write(
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
        "workspace/packages/app/src/lib.ql",
        "pub fn app_value() -> Int { return 1 }\n",
    );
    temp.write(
        "workspace/packages/tool/src/lib.ql",
        "pub fn tool_value() -> Int { return 2 }\n",
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["build"])
        .arg(&project_root)
        .args(["--package", "missing", "--json"]);
    let output = run_command_capture(&mut command, "`ql build --json --package missing`");
    let (stdout, stderr) = expect_exit_code(
        "project-build-json-selector-miss",
        "selector miss build json failure",
        &output,
        1,
    )
    .expect("project-path `ql build --json --package missing` should exit with code 1");
    expect_empty_stderr(
        "project-build-json-selector-miss",
        "selector miss build json failure",
        &stderr,
    )
    .expect("project-path `ql build --json --package missing` should not print stderr");

    let json = parse_json_output("project-build-json-selector-miss", &stdout);
    assert_eq!(json["schema"], "ql.build.v1");
    assert_eq!(
        json["path"],
        project_root.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["scope"], "project");
    assert_eq!(
        json["project_manifest_path"],
        workspace_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["status"], "failed");
    assert_eq!(json["built_targets"], serde_json::json!([]));
    assert_eq!(json["interfaces"], serde_json::json!([]));
    assert_eq!(json["failure"]["manifest_path"], JsonValue::Null);
    assert_eq!(json["failure"]["package_name"], JsonValue::Null);
    assert_eq!(json["failure"]["selected"], JsonValue::Null);
    assert_eq!(json["failure"]["dependency_only"], JsonValue::Null);
    assert_eq!(json["failure"]["kind"], JsonValue::Null);
    assert_eq!(
        json["failure"]["path"],
        project_root.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["failure"]["error_kind"], "selector");
    assert_eq!(json["failure"]["stage"], "target-selection");
    assert_eq!(json["failure"]["selector"], "package `missing`");
    assert!(
        json["failure"]["message"]
            .as_str()
            .expect("selector miss json failure should expose a message")
            .contains("target selector matched no build targets"),
        "selector miss json failure should describe the selector mismatch: {json}"
    );
}

#[test]
fn build_package_path_json_reports_target_failure_after_partial_build() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-package-json-failure");
    let dep_root = temp.path().join("dep");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(dep_root.join("src")).expect("create dependency source tree");
    std::fs::create_dir_all(project_root.join("src")).expect("create package source tree");

    let dep_manifest = temp.write(
        "dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write("dep/src/lib.ql", "pub fn exported() -> Int { return 1 }\n");
    let app_manifest = temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"

[dependencies]
dep = "../dep"
"#,
    );
    temp.write("app/src/lib.ql", "pub fn helper() -> Int { return 1 }\n");
    let app_main_path = temp.write("app/src/main.ql", "fn main() -> Int { return \"oops\" }\n");

    let dep_output = static_library_output_path(&dep_root.join("target/ql/debug"), "lib");
    let app_lib_output = static_library_output_path(&project_root.join("target/ql/debug"), "lib");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["build"]).arg(&project_root).arg("--json");
    let output = run_command_capture(
        &mut command,
        "`ql build --json` package target failure after partial build",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-build-package-json-failure",
        "package target failure after partial build json",
        &output,
        1,
    )
    .expect("package-path `ql build --json` target failure should exit with code 1");
    expect_empty_stderr(
        "project-build-package-json-failure",
        "package target failure after partial build json",
        &stderr,
    )
    .expect("package-path `ql build --json` target failure should not print stderr");

    let json = parse_json_output("project-build-package-json-failure", &stdout);
    assert_eq!(json["schema"], "ql.build.v1");
    assert_eq!(
        json["path"],
        project_root.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["scope"], "project");
    assert_eq!(
        json["project_manifest_path"],
        app_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["status"], "failed");
    assert_eq!(json["requested_emit"], "llvm-ir");
    assert_eq!(json["requested_profile"], "debug");
    assert_eq!(json["profile_overridden"], false);
    assert_eq!(json["emit_interface"], false);

    let built_targets = json["built_targets"]
        .as_array()
        .expect("build failure json should expose built_targets");
    assert_eq!(built_targets.len(), 2);
    assert_eq!(
        built_targets[0]["manifest_path"],
        dep_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(built_targets[0]["package_name"], "dep");
    assert_eq!(built_targets[0]["selected"], false);
    assert_eq!(built_targets[0]["dependency_only"], true);
    assert_eq!(built_targets[0]["kind"], "lib");
    assert_eq!(built_targets[0]["path"], "src/lib.ql");
    assert_eq!(built_targets[0]["emit"], "staticlib");
    assert_eq!(
        built_targets[0]["artifact_path"],
        dep_output.display().to_string().replace('\\', "/")
    );
    assert_eq!(
        built_targets[1]["manifest_path"],
        app_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(built_targets[1]["package_name"], "app");
    assert_eq!(built_targets[1]["selected"], true);
    assert_eq!(built_targets[1]["dependency_only"], false);
    assert_eq!(built_targets[1]["kind"], "lib");
    assert_eq!(built_targets[1]["path"], "src/lib.ql");
    assert_eq!(built_targets[1]["emit"], "staticlib");
    assert_eq!(
        built_targets[1]["artifact_path"],
        app_lib_output.display().to_string().replace('\\', "/")
    );

    assert_eq!(json["interfaces"], serde_json::json!([]));
    assert_eq!(
        json["failure"]["manifest_path"],
        app_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["failure"]["package_name"], "app");
    assert_eq!(json["failure"]["selected"], true);
    assert_eq!(json["failure"]["dependency_only"], false);
    assert_eq!(json["failure"]["kind"], "bin");
    assert_eq!(json["failure"]["path"], "src/main.ql");
    assert_eq!(json["failure"]["error_kind"], "diagnostics");
    assert_eq!(json["failure"]["message"], "build produced diagnostics");
    assert_eq!(
        json["failure"]["diagnostic_file"]["path"],
        app_main_path.display().to_string().replace('\\', "/")
    );
    assert_eq!(
        json["failure"]["diagnostic_file"]["diagnostics"][0]["message"],
        "return value has type mismatch: expected `Int`, found `String`"
    );

    expect_file_exists(
        "project-build-package-json-failure",
        &dep_output,
        "dependency package artifact",
        "package target failure after partial build json",
    )
    .expect("package-path `ql build --json` should preserve the dependency artifact");
    expect_file_exists(
        "project-build-package-json-failure",
        &app_lib_output,
        "package library artifact",
        "package target failure after partial build json",
    )
    .expect(
        "package-path `ql build --json` should preserve the already-built package library artifact",
    );
}

#[test]
fn build_package_path_json_reports_target_prep_dependency_extern_conflict() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-package-json-target-prep-extern-conflict");
    let dep_a_root = temp.path().join("dep-a");
    let dep_b_root = temp.path().join("dep-b");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(dep_a_root.join("src"))
        .expect("create dep-a source tree for target-prep extern conflict");
    std::fs::create_dir_all(dep_b_root.join("src"))
        .expect("create dep-b source tree for target-prep extern conflict");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create app source tree for target-prep extern conflict");

    let dep_a_manifest = temp.write(
        "dep-a/qlang.toml",
        r#"
[package]
name = "demo.shared.alpha"
"#,
    );
    temp.write(
        "dep-a/src/lib.ql",
        "extern \"c\" pub fn q_shared() -> Int { return 1 }\n",
    );
    let dep_b_manifest = temp.write(
        "dep-b/qlang.toml",
        r#"
[package]
name = "demo.shared.beta"
"#,
    );
    temp.write(
        "dep-b/src/lib.ql",
        "extern \"c\" pub fn q_shared() -> Int { return 2 }\n",
    );
    let app_manifest = temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"

[dependencies]
alpha = "../dep-a"
beta = "../dep-b"
"#,
    );
    temp.write(
        "app/src/main.ql",
        "use demo.shared.alpha.q_shared as alpha_shared\nuse demo.shared.beta.q_shared as beta_shared\n\nfn main() -> Int { return alpha_shared() + beta_shared() }\n",
    );

    let dep_a_output = static_library_output_path(&dep_a_root.join("target/ql/debug"), "lib");
    let dep_b_output = static_library_output_path(&dep_b_root.join("target/ql/debug"), "lib");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["build"]).arg(&project_root).arg("--json");
    let output = run_command_capture(
        &mut command,
        "`ql build --json` target-prep dependency extern conflict",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-build-package-json-target-prep-extern-conflict",
        "package build json target-prep dependency extern conflict",
        &output,
        1,
    )
    .expect(
        "package-path `ql build --json` should fail on target-prep dependency extern conflicts",
    );
    expect_empty_stderr(
        "project-build-package-json-target-prep-extern-conflict",
        "package build json target-prep dependency extern conflict",
        &stderr,
    )
    .expect("target-prep dependency extern conflicts should stay on stdout in json mode");

    let json = parse_json_output(
        "project-build-package-json-target-prep-extern-conflict",
        &stdout,
    );
    assert_eq!(json["schema"], "ql.build.v1");
    assert_eq!(
        json["path"],
        project_root.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["scope"], "project");
    assert_eq!(
        json["project_manifest_path"],
        app_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["status"], "failed");

    let built_targets = json["built_targets"]
        .as_array()
        .expect("target-prep conflict json should expose built_targets");
    assert_eq!(built_targets.len(), 2);
    assert_eq!(
        built_targets[0]["manifest_path"],
        dep_a_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(built_targets[0]["package_name"], "demo.shared.alpha");
    assert_eq!(built_targets[0]["selected"], false);
    assert_eq!(built_targets[0]["dependency_only"], true);
    assert_eq!(built_targets[0]["kind"], "lib");
    assert_eq!(
        built_targets[0]["artifact_path"],
        dep_a_output.display().to_string().replace('\\', "/")
    );
    assert_eq!(
        built_targets[1]["manifest_path"],
        dep_b_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(built_targets[1]["package_name"], "demo.shared.beta");
    assert_eq!(built_targets[1]["selected"], false);
    assert_eq!(built_targets[1]["dependency_only"], true);
    assert_eq!(built_targets[1]["kind"], "lib");
    assert_eq!(
        built_targets[1]["artifact_path"],
        dep_b_output.display().to_string().replace('\\', "/")
    );

    assert_eq!(json["interfaces"], serde_json::json!([]));
    assert_eq!(
        json["failure"]["manifest_path"],
        app_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["failure"]["package_name"], "app");
    assert_eq!(json["failure"]["selected"], true);
    assert_eq!(json["failure"]["dependency_only"], false);
    assert_eq!(json["failure"]["kind"], "bin");
    assert_eq!(json["failure"]["path"], "src/main.ql");
    assert_eq!(json["failure"]["stage"], "target-prep");
    assert_eq!(json["failure"]["error_kind"], "dependency-extern-conflict");
    assert_eq!(json["failure"]["symbol"], "q_shared");
    assert_eq!(
        json["failure"]["first_dependency_package"],
        "demo.shared.alpha"
    );
    assert_eq!(
        json["failure"]["first_dependency_manifest_path"],
        dep_a_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(
        json["failure"]["conflicting_dependency_package"],
        "demo.shared.beta"
    );
    assert_eq!(
        json["failure"]["conflicting_dependency_manifest_path"],
        dep_b_manifest.display().to_string().replace('\\', "/")
    );
    assert!(
        json["failure"]["message"]
            .as_str()
            .expect("target-prep conflict json should expose a message")
            .contains("conflicting direct dependency extern imports"),
        "target-prep conflict json should preserve the extern collision detail: {json}"
    );

    expect_file_exists(
        "project-build-package-json-target-prep-extern-conflict",
        &dep_a_output,
        "dep-a artifact",
        "package build json target-prep dependency extern conflict",
    )
    .expect("target-prep conflict should preserve dep-a artifact");
    expect_file_exists(
        "project-build-package-json-target-prep-extern-conflict",
        &dep_b_output,
        "dep-b artifact",
        "package build json target-prep dependency extern conflict",
    )
    .expect("target-prep conflict should preserve dep-b artifact");
}

#[test]
fn build_package_path_json_reports_dependency_public_function_local_conflict() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-package-json-public-function-local-conflict");
    let dep_root = temp.path().join("dep");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(dep_root.join("src"))
        .expect("create dep source tree for dependency public function local conflict");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create app source tree for dependency public function local conflict");

    let dep_manifest = temp.write(
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
    let app_manifest = temp.write(
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
        "use dep.add as sum\n\nfn add(left: Int, right: Int) -> Int { return left - right }\n\nfn main() -> Int { return sum(8, 5) }\n",
    );

    let dep_output = static_library_output_path(&dep_root.join("target/ql/debug"), "lib");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["build"]).arg(&project_root).arg("--json");
    let output = run_command_capture(
        &mut command,
        "`ql build --json` dependency public function local conflict",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-build-package-json-public-function-local-conflict",
        "package build json dependency public function local conflict",
        &output,
        1,
    )
    .expect(
        "package-path `ql build --json` should fail when the root source already defines the dependency bridge symbol",
    );
    expect_empty_stderr(
        "project-build-package-json-public-function-local-conflict",
        "package build json dependency public function local conflict",
        &stderr,
    )
    .expect("dependency public function local conflicts should stay on stdout in json mode");

    let json = parse_json_output(
        "project-build-package-json-public-function-local-conflict",
        &stdout,
    );
    assert_eq!(json["schema"], "ql.build.v1");
    assert_eq!(
        json["path"],
        project_root.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["scope"], "project");
    assert_eq!(
        json["project_manifest_path"],
        app_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["status"], "failed");

    let built_targets = json["built_targets"]
        .as_array()
        .expect("local conflict json should expose built_targets");
    assert_eq!(built_targets.len(), 1);
    assert_eq!(
        built_targets[0]["manifest_path"],
        dep_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(built_targets[0]["package_name"], "dep");
    assert_eq!(built_targets[0]["selected"], false);
    assert_eq!(built_targets[0]["dependency_only"], true);
    assert_eq!(built_targets[0]["kind"], "lib");
    assert_eq!(
        built_targets[0]["artifact_path"],
        dep_output.display().to_string().replace('\\', "/")
    );

    assert_eq!(json["interfaces"], serde_json::json!([]));
    assert_eq!(
        json["failure"]["manifest_path"],
        app_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["failure"]["package_name"], "app");
    assert_eq!(json["failure"]["selected"], true);
    assert_eq!(json["failure"]["dependency_only"], false);
    assert_eq!(json["failure"]["kind"], "bin");
    assert_eq!(json["failure"]["path"], "src/main.ql");
    assert_eq!(json["failure"]["stage"], "target-prep");
    assert_eq!(
        json["failure"]["error_kind"],
        "dependency-function-local-conflict"
    );
    assert_eq!(json["failure"]["symbol"], "add");
    assert_eq!(json["failure"]["dependency_package"], "dep");
    assert_eq!(
        json["failure"]["dependency_manifest_path"],
        dep_manifest.display().to_string().replace('\\', "/")
    );
    assert!(
        json["failure"]["message"]
            .as_str()
            .expect("local conflict json should expose a message")
            .contains("already defines the same top-level name"),
        "local conflict json should preserve the bridge-name collision detail: {json}"
    );

    expect_file_exists(
        "project-build-package-json-public-function-local-conflict",
        &dep_output,
        "dependency package artifact",
        "package build json dependency public function local conflict",
    )
    .expect("local conflict should preserve the dependency package artifact");
}

#[test]
fn build_package_path_json_reports_dependency_public_value_local_conflict() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-package-json-public-value-local-conflict");
    let dep_root = temp.path().join("dep");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(dep_root.join("src"))
        .expect("create dep source tree for dependency public value local conflict");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create app source tree for dependency public value local conflict");

    let dep_manifest = temp.write(
        "dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write("dep/src/lib.ql", "pub const VALUE: Int = 7\n");
    let app_manifest = temp.write(
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
        "use dep.VALUE as VALUE_ALIAS\n\nconst VALUE: Int = 2\n\nfn main() -> Int { return VALUE_ALIAS }\n",
    );

    let dep_output = static_library_output_path(&dep_root.join("target/ql/debug"), "lib");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["build"]).arg(&project_root).arg("--json");
    let output = run_command_capture(
        &mut command,
        "`ql build --json` dependency public value local conflict",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-build-package-json-public-value-local-conflict",
        "package build json dependency public value local conflict",
        &output,
        1,
    )
    .expect(
        "package-path `ql build --json` should fail when the root source already defines the dependency public value bridge symbol",
    );
    expect_empty_stderr(
        "project-build-package-json-public-value-local-conflict",
        "package build json dependency public value local conflict",
        &stderr,
    )
    .expect("dependency public value local conflicts should stay on stdout in json mode");

    let json = parse_json_output(
        "project-build-package-json-public-value-local-conflict",
        &stdout,
    );
    assert_eq!(json["schema"], "ql.build.v1");
    assert_eq!(
        json["path"],
        project_root.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["scope"], "project");
    assert_eq!(
        json["project_manifest_path"],
        app_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["status"], "failed");

    let built_targets = json["built_targets"]
        .as_array()
        .expect("local conflict json should expose built_targets");
    assert_eq!(built_targets.len(), 1);
    assert_eq!(
        built_targets[0]["manifest_path"],
        dep_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(built_targets[0]["package_name"], "dep");
    assert_eq!(built_targets[0]["selected"], false);
    assert_eq!(built_targets[0]["dependency_only"], true);
    assert_eq!(built_targets[0]["kind"], "lib");
    assert_eq!(
        built_targets[0]["artifact_path"],
        dep_output.display().to_string().replace('\\', "/")
    );

    assert_eq!(json["interfaces"], serde_json::json!([]));
    assert_eq!(
        json["failure"]["manifest_path"],
        app_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["failure"]["package_name"], "app");
    assert_eq!(json["failure"]["selected"], true);
    assert_eq!(json["failure"]["dependency_only"], false);
    assert_eq!(json["failure"]["kind"], "bin");
    assert_eq!(json["failure"]["path"], "src/main.ql");
    assert_eq!(json["failure"]["stage"], "target-prep");
    assert_eq!(
        json["failure"]["error_kind"],
        "dependency-value-local-conflict"
    );
    assert_eq!(json["failure"]["symbol"], "VALUE");
    assert_eq!(json["failure"]["dependency_package"], "dep");
    assert_eq!(
        json["failure"]["dependency_manifest_path"],
        dep_manifest.display().to_string().replace('\\', "/")
    );
    assert!(
        json["failure"]["message"]
            .as_str()
            .expect("local conflict json should expose a message")
            .contains("already defines the same top-level name"),
        "local conflict json should preserve the bridge-name collision detail: {json}"
    );

    expect_file_exists(
        "project-build-package-json-public-value-local-conflict",
        &dep_output,
        "dependency package artifact",
        "package build json dependency public value local conflict",
    )
    .expect("local conflict should preserve the dependency package artifact");
}

#[test]
fn build_package_path_json_reports_implicit_dependency_public_function_local_conflict() {
    let workspace_root = workspace_root();
    let temp =
        TempDir::new("ql-project-build-package-json-implicit-public-function-local-conflict");
    let dep_root = temp.path().join("dep");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(dep_root.join("src"))
        .expect("create dep source tree for implicit dependency public function local conflict");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create app source tree for implicit dependency public function local conflict");

    let dep_manifest = temp.write(
        "dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "dep/src/lib.ql",
        "pub fn add_one(value: Int) -> Int { return value + 1 }\npub const APPLY: (Int) -> Int = add_one\n",
    );
    let app_manifest = temp.write(
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
        "use dep.APPLY as RUN\n\nfn add_one(value: Int) -> Int { return value - 1 }\n\nfn main() -> Int { return RUN(8) }\n",
    );

    let dep_output = static_library_output_path(&dep_root.join("target/ql/debug"), "lib");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["build"]).arg(&project_root).arg("--json");
    let output = run_command_capture(
        &mut command,
        "`ql build --json` implicit dependency public function local conflict",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-build-package-json-implicit-public-function-local-conflict",
        "package build json implicit dependency public function local conflict",
        &output,
        1,
    )
    .expect(
        "package-path `ql build --json` should fail when an implicit dependency function bridge collides with a local top-level function",
    );
    expect_empty_stderr(
        "project-build-package-json-implicit-public-function-local-conflict",
        "package build json implicit dependency public function local conflict",
        &stderr,
    )
    .expect(
        "implicit dependency public function local conflicts should stay on stdout in json mode",
    );

    let json = parse_json_output(
        "project-build-package-json-implicit-public-function-local-conflict",
        &stdout,
    );
    assert_eq!(json["schema"], "ql.build.v1");
    assert_eq!(
        json["path"],
        project_root.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["scope"], "project");
    assert_eq!(
        json["project_manifest_path"],
        app_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["status"], "failed");

    let built_targets = json["built_targets"]
        .as_array()
        .expect("implicit local conflict json should expose built_targets");
    assert_eq!(built_targets.len(), 1);
    assert_eq!(
        built_targets[0]["manifest_path"],
        dep_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(built_targets[0]["package_name"], "dep");
    assert_eq!(built_targets[0]["selected"], false);
    assert_eq!(built_targets[0]["dependency_only"], true);
    assert_eq!(built_targets[0]["kind"], "lib");
    assert_eq!(
        built_targets[0]["artifact_path"],
        dep_output.display().to_string().replace('\\', "/")
    );

    assert_eq!(json["interfaces"], serde_json::json!([]));
    assert_eq!(
        json["failure"]["manifest_path"],
        app_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["failure"]["package_name"], "app");
    assert_eq!(json["failure"]["selected"], true);
    assert_eq!(json["failure"]["dependency_only"], false);
    assert_eq!(json["failure"]["kind"], "bin");
    assert_eq!(json["failure"]["path"], "src/main.ql");
    assert_eq!(json["failure"]["stage"], "target-prep");
    assert_eq!(
        json["failure"]["error_kind"],
        "dependency-function-local-conflict"
    );
    assert_eq!(json["failure"]["symbol"], "add_one");
    assert_eq!(json["failure"]["dependency_package"], "dep");
    assert_eq!(
        json["failure"]["dependency_manifest_path"],
        dep_manifest.display().to_string().replace('\\', "/")
    );
    assert!(
        json["failure"]["message"]
            .as_str()
            .expect("implicit local conflict json should expose a message")
            .contains("already defines the same top-level name"),
        "implicit local conflict json should preserve the bridge-name collision detail: {json}"
    );

    expect_file_exists(
        "project-build-package-json-implicit-public-function-local-conflict",
        &dep_output,
        "dependency package artifact",
        "package build json implicit dependency public function local conflict",
    )
    .expect("implicit local conflict should preserve the dependency package artifact");
}

#[test]
fn build_package_path_json_reports_implicit_dependency_public_type_local_conflict() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-package-json-implicit-public-type-local-conflict");
    let dep_root = temp.path().join("dep");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(dep_root.join("src"))
        .expect("create dep source tree for implicit dependency public type local conflict");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create app source tree for implicit dependency public type local conflict");

    let dep_manifest = temp.write(
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
    let app_manifest = temp.write(
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
        "use dep.make_box as make\n\nstruct Box { value: Int }\n\nfn main() -> Int {\n    let value = make()\n    return value.value\n}\n",
    );

    let dep_output = static_library_output_path(&dep_root.join("target/ql/debug"), "lib");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["build"]).arg(&project_root).arg("--json");
    let output = run_command_capture(
        &mut command,
        "`ql build --json` implicit dependency public type local conflict",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-build-package-json-implicit-public-type-local-conflict",
        "package build json implicit dependency public type local conflict",
        &output,
        1,
    )
    .expect(
        "package-path `ql build --json` should fail when an implicit dependency type bridge collides with a local top-level type",
    );
    expect_empty_stderr(
        "project-build-package-json-implicit-public-type-local-conflict",
        "package build json implicit dependency public type local conflict",
        &stderr,
    )
    .expect("implicit dependency public type local conflicts should stay on stdout in json mode");

    let json = parse_json_output(
        "project-build-package-json-implicit-public-type-local-conflict",
        &stdout,
    );
    assert_eq!(json["schema"], "ql.build.v1");
    assert_eq!(
        json["path"],
        project_root.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["scope"], "project");
    assert_eq!(
        json["project_manifest_path"],
        app_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["status"], "failed");

    let built_targets = json["built_targets"]
        .as_array()
        .expect("implicit type local conflict json should expose built_targets");
    assert_eq!(built_targets.len(), 1);
    assert_eq!(
        built_targets[0]["manifest_path"],
        dep_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(built_targets[0]["package_name"], "dep");
    assert_eq!(built_targets[0]["selected"], false);
    assert_eq!(built_targets[0]["dependency_only"], true);
    assert_eq!(built_targets[0]["kind"], "lib");
    assert_eq!(
        built_targets[0]["artifact_path"],
        dep_output.display().to_string().replace('\\', "/")
    );

    assert_eq!(json["interfaces"], serde_json::json!([]));
    assert_eq!(
        json["failure"]["manifest_path"],
        app_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["failure"]["package_name"], "app");
    assert_eq!(json["failure"]["selected"], true);
    assert_eq!(json["failure"]["dependency_only"], false);
    assert_eq!(json["failure"]["kind"], "bin");
    assert_eq!(json["failure"]["path"], "src/main.ql");
    assert_eq!(json["failure"]["stage"], "target-prep");
    assert_eq!(
        json["failure"]["error_kind"],
        "dependency-type-local-conflict"
    );
    assert_eq!(json["failure"]["symbol"], "Box");
    assert_eq!(json["failure"]["dependency_package"], "dep");
    assert_eq!(
        json["failure"]["dependency_manifest_path"],
        dep_manifest.display().to_string().replace('\\', "/")
    );
    assert!(
        json["failure"]["message"]
            .as_str()
            .expect("implicit type local conflict json should expose a message")
            .contains("already defines the same top-level name"),
        "implicit type local conflict json should preserve the bridge-name collision detail: {json}"
    );

    expect_file_exists(
        "project-build-package-json-implicit-public-type-local-conflict",
        &dep_output,
        "dependency package artifact",
        "package build json implicit dependency public type local conflict",
    )
    .expect("implicit type local conflict should preserve the dependency package artifact");
}

#[test]
fn build_package_path_json_ignores_unused_dependency_extern_conflicts() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-package-json-unused-extern-conflict");
    let dep_a_root = temp.path().join("dep-a");
    let dep_b_root = temp.path().join("dep-b");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(dep_a_root.join("src"))
        .expect("create dep-a source tree for unused extern conflict");
    std::fs::create_dir_all(dep_b_root.join("src"))
        .expect("create dep-b source tree for unused extern conflict");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create app source tree for unused extern conflict");

    let dep_a_manifest = temp.write(
        "dep-a/qlang.toml",
        r#"
[package]
name = "demo.shared.alpha"
"#,
    );
    temp.write(
        "dep-a/src/lib.ql",
        "extern \"c\" pub fn q_shared() -> Int { return 1 }\n",
    );
    let dep_b_manifest = temp.write(
        "dep-b/qlang.toml",
        r#"
[package]
name = "demo.shared.beta"
"#,
    );
    temp.write(
        "dep-b/src/lib.ql",
        "extern \"c\" pub fn q_shared() -> Int { return 2 }\n",
    );
    let app_manifest = temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"

[dependencies]
alpha = "../dep-a"
beta = "../dep-b"
"#,
    );
    temp.write(
        "app/src/main.ql",
        "use demo.shared.alpha.q_shared as shared\n\nfn main() -> Int { return shared() }\n",
    );

    let dep_a_output = static_library_output_path(&dep_a_root.join("target/ql/debug"), "lib");
    let dep_b_output = static_library_output_path(&dep_b_root.join("target/ql/debug"), "lib");
    let app_output = project_root.join("target/ql/debug/main.ll");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["build"]).arg(&project_root).arg("--json");
    let output = run_command_capture(
        &mut command,
        "`ql build --json` ignores unused dependency extern conflicts",
    );
    let (stdout, stderr) = expect_success(
        "project-build-package-json-unused-extern-conflict",
        "package build json unused dependency extern conflict",
        &output,
    )
    .expect("package-path `ql build --json` should ignore unused dependency extern conflicts");
    expect_empty_stderr(
        "project-build-package-json-unused-extern-conflict",
        "package build json unused dependency extern conflict",
        &stderr,
    )
    .expect("unused dependency extern conflict build should not print stderr");

    let json = parse_json_output("project-build-package-json-unused-extern-conflict", &stdout);
    assert_eq!(json["schema"], "ql.build.v1");
    assert_eq!(
        json["path"],
        project_root.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["scope"], "project");
    assert_eq!(
        json["project_manifest_path"],
        app_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["status"], "ok");
    assert_eq!(json["failure"], JsonValue::Null);
    let built_targets = json["built_targets"]
        .as_array()
        .expect("unused dependency extern conflict json should expose built_targets");
    assert_eq!(built_targets.len(), 3);
    assert_eq!(
        built_targets[0],
        serde_json::json!({
            "manifest_path": dep_a_manifest.display().to_string().replace('\\', "/"),
            "package_name": "demo.shared.alpha",
            "selected": false,
            "dependency_only": true,
            "kind": "lib",
            "path": "src/lib.ql",
            "emit": "staticlib",
            "profile": "debug",
            "artifact_path": dep_a_output.display().to_string().replace('\\', "/"),
            "c_header_path": JsonValue::Null,
        })
    );
    assert_eq!(
        built_targets[1],
        serde_json::json!({
            "manifest_path": dep_b_manifest.display().to_string().replace('\\', "/"),
            "package_name": "demo.shared.beta",
            "selected": false,
            "dependency_only": true,
            "kind": "lib",
            "path": "src/lib.ql",
            "emit": "staticlib",
            "profile": "debug",
            "artifact_path": dep_b_output.display().to_string().replace('\\', "/"),
            "c_header_path": JsonValue::Null,
        })
    );
    assert_eq!(
        built_targets[2],
        serde_json::json!({
            "manifest_path": app_manifest.display().to_string().replace('\\', "/"),
            "package_name": "app",
            "selected": true,
            "dependency_only": false,
            "kind": "bin",
            "path": "src/main.ql",
            "emit": "llvm-ir",
            "profile": "debug",
            "artifact_path": app_output.display().to_string().replace('\\', "/"),
            "c_header_path": JsonValue::Null,
        })
    );

    expect_file_exists(
        "project-build-package-json-unused-extern-conflict",
        &dep_a_output,
        "dep-a artifact",
        "package build json unused dependency extern conflict",
    )
    .expect("unused dependency extern conflict build should preserve dep-a artifact");
    expect_file_exists(
        "project-build-package-json-unused-extern-conflict",
        &dep_b_output,
        "dep-b artifact",
        "package build json unused dependency extern conflict",
    )
    .expect("unused dependency extern conflict build should preserve dep-b artifact");
    expect_file_exists(
        "project-build-package-json-unused-extern-conflict",
        &app_output,
        "app artifact",
        "package build json unused dependency extern conflict",
    )
    .expect("unused dependency extern conflict build should emit the selected artifact");
}

#[test]
fn build_project_json_reports_emit_interface_output_failure() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-json-emit-interface-output-failure");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create package source tree for json emit-interface output failure test");
    let manifest_path = temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write("app/src/lib.ql", "pub fn util() -> Int { return 1 }\n");
    temp.write("app/src/main.ql", "fn main() -> Int { return 0 }\n");

    let lib_output = static_library_output_path(&project_root.join("target/ql/debug"), "lib");
    let main_output = project_root.join("target/ql/debug/main.ll");
    let interface_output = project_root.join("app.qi");
    std::fs::create_dir_all(&interface_output)
        .expect("create blocking interface directory for json emit-interface output failure test");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["build"])
        .arg(&project_root)
        .args(["--emit-interface", "--json"]);
    let output = run_command_capture(
        &mut command,
        "`ql build --emit-interface --json` blocked interface output path",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-build-json-emit-interface-output-failure",
        "project build json emit-interface output failure",
        &output,
        1,
    )
    .expect("project-path `ql build --emit-interface --json` should exit with code 1");
    expect_empty_stderr(
        "project-build-json-emit-interface-output-failure",
        "project build json emit-interface output failure",
        &stderr,
    )
    .expect("project-path `ql build --emit-interface --json` should not print stderr");

    let json = parse_json_output("project-build-json-emit-interface-output-failure", &stdout);
    assert_eq!(json["schema"], "ql.build.v1");
    assert_eq!(
        json["path"],
        project_root.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["scope"], "project");
    assert_eq!(
        json["project_manifest_path"],
        manifest_path.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["status"], "failed");
    assert_eq!(json["emit_interface"], true);
    assert_eq!(json["interfaces"], serde_json::json!([]));
    assert_eq!(
        json["failure"]["manifest_path"],
        manifest_path.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["failure"]["package_name"], "app");
    assert_eq!(json["failure"]["selected"], true);
    assert_eq!(json["failure"]["dependency_only"], false);
    assert_eq!(json["failure"]["kind"], "interface");
    assert_eq!(
        json["failure"]["path"],
        interface_output.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["failure"]["error_kind"], "interface-output");
    assert_eq!(json["failure"]["stage"], "emit-interface");
    assert_eq!(
        json["failure"]["output_path"],
        interface_output.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["failure"]["source_root"], JsonValue::Null);
    assert_eq!(json["failure"]["failing_source_count"], JsonValue::Null);
    assert_eq!(json["failure"]["first_failing_source"], JsonValue::Null);
    assert!(
        json["failure"]["message"]
            .as_str()
            .expect("emit-interface output failure should expose a message")
            .contains("failed to write interface"),
        "emit-interface output failure should preserve the interface write failure: {json}"
    );

    let built_targets = json["built_targets"]
        .as_array()
        .expect("emit-interface output failure should expose built_targets");
    assert_eq!(built_targets.len(), 2);
    assert_eq!(
        built_targets[0]["artifact_path"],
        lib_output.display().to_string().replace('\\', "/")
    );
    assert_eq!(
        built_targets[1]["artifact_path"],
        main_output.display().to_string().replace('\\', "/")
    );
    expect_file_exists(
        "project-build-json-emit-interface-output-failure",
        &lib_output,
        "package library artifact",
        "project build json emit-interface output failure",
    )
    .expect("project-path `ql build --emit-interface --json` should preserve the library artifact");
    expect_file_exists(
        "project-build-json-emit-interface-output-failure",
        &main_output,
        "package binary artifact",
        "project build json emit-interface output failure",
    )
    .expect("project-path `ql build --emit-interface --json` should preserve the binary artifact");
    assert!(
        interface_output.is_dir(),
        "project-path `ql build --emit-interface --json` should not replace the blocked interface output directory"
    );
}
