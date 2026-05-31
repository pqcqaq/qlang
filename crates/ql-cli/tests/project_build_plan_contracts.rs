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
