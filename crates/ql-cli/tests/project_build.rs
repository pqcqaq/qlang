mod support;

use serde_json::Value as JsonValue;
use support::{
    TempDir, expect_empty_stderr, expect_empty_stdout, expect_exit_code, expect_file_exists,
    expect_stdout_contains_all, expect_success, ql_command, run_command_capture,
    static_library_output_path, workspace_root,
};

fn normalize_output_text(text: &str) -> String {
    text.replace("\r\n", "\n")
}

fn parse_json_output(case_name: &str, stdout: &str) -> JsonValue {
    serde_json::from_str(&normalize_output_text(stdout))
        .unwrap_or_else(|error| panic!("[{case_name}] parse json stdout: {error}\n{stdout}"))
}

#[test]
fn build_package_path_builds_all_discovered_targets() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-package");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src/bin/tools"))
        .expect("create package source tree for build test");
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write("app/src/lib.ql", "pub fn util() -> Int { return 1 }\n");
    temp.write("app/src/main.ql", "fn main() -> Int { return 0 }\n");
    temp.write(
        "app/src/bin/tools/repl.ql",
        "fn main() -> Int { return 2 }\n",
    );

    let lib_output = static_library_output_path(&project_root.join("target/ql/debug"), "lib");
    let main_output = project_root.join("target/ql/debug/main.ll");
    let repl_output = project_root.join("target/ql/debug/bin/tools/repl.ll");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["build"]).arg(&project_root);
    let output = run_command_capture(&mut command, "`ql build` package path");
    let (stdout, stderr) = expect_success("project-build-package", "package path build", &output)
        .expect("package path build should succeed");
    expect_empty_stderr("project-build-package", "package path build", &stderr)
        .expect("package path build should not print stderr");
    expect_stdout_contains_all(
        "project-build-package",
        &stdout.replace('\\', "/"),
        &[
            &format!("wrote staticlib: {}", lib_output.display()).replace('\\', "/"),
            &format!("wrote llvm-ir: {}", main_output.display()).replace('\\', "/"),
            &format!("wrote llvm-ir: {}", repl_output.display()).replace('\\', "/"),
        ],
    )
    .expect("package path build should report every discovered target artifact");

    expect_file_exists(
        "project-build-package",
        &lib_output,
        "package library artifact",
        "package path build",
    )
    .expect("package path build should emit the library artifact");
    expect_file_exists(
        "project-build-package",
        &main_output,
        "package binary artifact",
        "package path build",
    )
    .expect("package path build should emit the main artifact");
    expect_file_exists(
        "project-build-package",
        &repl_output,
        "package nested bin artifact",
        "package path build",
    )
    .expect("package path build should emit nested bin artifacts under a stable relative path");
}

#[test]
fn build_project_source_file_uses_project_aware_dependency_plan() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-source-file-project-aware");
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
    temp.write(
        "dep/src/lib.ql",
        "extern \"c\" pub fn q_add(left: Int, right: Int) -> Int { return left + right }\n",
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
    let app_main = temp.write(
        "app/src/main.ql",
        "use dep.q_add as add\n\nfn main() -> Int { return add(6, 7) }\n",
    );

    let dep_output = static_library_output_path(&dep_root.join("target/ql/debug"), "lib");
    let app_output = project_root.join("target/ql/debug/main.ll");
    let interface_output = dep_root.join("dep.qi");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["build"]).arg(&app_main).arg("--json");
    let output = run_command_capture(&mut command, "`ql build --json` direct project source file");
    let (stdout, stderr) = expect_success(
        "project-build-source-file-project-aware",
        "direct project source file json build",
        &output,
    )
    .expect("direct project source file `ql build --json` should succeed");
    expect_empty_stderr(
        "project-build-source-file-project-aware",
        "direct project source file json build",
        &stderr,
    )
    .expect("direct project source file `ql build --json` should not print stderr");

    let json = parse_json_output("project-build-source-file-project-aware", &stdout);
    assert_eq!(json["schema"], "ql.build.v1");
    assert_eq!(json["scope"], "project");
    assert_eq!(
        json["path"],
        app_main.display().to_string().replace('\\', "/")
    );
    assert_eq!(
        json["project_manifest_path"],
        app_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["status"], "ok");
    assert_eq!(
        json["interfaces"],
        serde_json::json!([
            {
                "manifest_path": app_manifest.display().to_string().replace('\\', "/"),
                "package_name": "app",
                "selected": true,
                "status": "wrote",
                "path": project_root.join("app.qi").display().to_string().replace('\\', "/"),
            }
        ])
    );
    let built_targets = json["built_targets"]
        .as_array()
        .expect("project-aware source build should expose built targets");
    assert_eq!(built_targets.len(), 2);
    assert_eq!(
        built_targets[0],
        serde_json::json!({
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
        })
    );
    assert_eq!(
        built_targets[1],
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
        "project-build-source-file-project-aware",
        &dep_output,
        "dependency package artifact",
        "direct project source file json build",
    )
    .expect("direct project source build should emit dependency artifacts");
    expect_file_exists(
        "project-build-source-file-project-aware",
        &app_output,
        "selected package artifact",
        "direct project source file json build",
    )
    .expect("direct project source build should emit the selected package artifact");
    expect_file_exists(
        "project-build-source-file-project-aware",
        &interface_output,
        "synced dependency interface",
        "direct project source file json build",
    )
    .expect("direct project source build should keep dependency interface sync");
}

#[test]
fn build_project_source_file_json_release_flag_overrides_manifest_default_profile() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-source-file-json-release-override");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create package source tree for direct project source release test");
    let app_manifest = temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"

[profile]
default = "debug"
"#,
    );
    let app_main = temp.write("app/src/main.ql", "fn main() -> Int { return 0 }\n");
    let app_output = project_root.join("target/ql/release/main.ll");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["build"])
        .arg(&app_main)
        .args(["--json", "--release"]);
    let output = run_command_capture(
        &mut command,
        "`ql build --json --release` direct project source file",
    );
    let (stdout, stderr) = expect_success(
        "project-build-source-file-json-release-override",
        "direct project source file release alias json build",
        &output,
    )
    .expect("direct project source file `ql build --json --release` should succeed");
    expect_empty_stderr(
        "project-build-source-file-json-release-override",
        "direct project source file release alias json build",
        &stderr,
    )
    .expect("direct project source file release alias json build should not print stderr");

    let json = parse_json_output("project-build-source-file-json-release-override", &stdout);
    assert_eq!(json["schema"], "ql.build.v1");
    assert_eq!(json["scope"], "project");
    assert_eq!(
        json["path"],
        app_main.display().to_string().replace('\\', "/")
    );
    assert_eq!(
        json["project_manifest_path"],
        app_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["requested_profile"], "release");
    assert_eq!(json["profile_overridden"], true);
    assert_eq!(json["status"], "ok");
    assert_eq!(
        json["built_targets"],
        serde_json::json!([
            {
                "manifest_path": app_manifest.display().to_string().replace('\\', "/"),
                "package_name": "app",
                "selected": true,
                "dependency_only": false,
                "kind": "bin",
                "path": "src/main.ql",
                "emit": "llvm-ir",
                "profile": "release",
                "artifact_path": app_output.display().to_string().replace('\\', "/"),
                "c_header_path": JsonValue::Null,
            }
        ])
    );
    expect_file_exists(
        "project-build-source-file-json-release-override",
        &app_output,
        "direct project source release alias artifact",
        "direct project source file release alias json build",
    )
    .expect("direct project source release alias json build should emit release artifact");
    assert!(
        !project_root.join("target/ql/debug/main.ll").exists(),
        "direct project source release alias json build should not emit debug artifact"
    );
}

#[test]
fn build_project_source_file_release_flag_overrides_manifest_default_profile() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-source-file-release-override");
    let dep_root = temp.path().join("dep");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(dep_root.join("src")).expect("create dependency source tree");
    std::fs::create_dir_all(project_root.join("src")).expect("create package source tree");

    let _dep_manifest = temp.write(
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
    let _app_manifest = temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"

[dependencies]
dep = "../dep"
"#,
    );
    let app_main = temp.write(
        "app/src/main.ql",
        "use dep.q_add as add\n\nfn main() -> Int { return add(6, 7) }\n",
    );

    let dep_output = static_library_output_path(&dep_root.join("target/ql/release"), "lib");
    let dep_stdout_output = project_root.join("../dep/target/ql/release");
    let app_output = project_root.join("target/ql/release/main.ll");
    let interface_output = dep_root.join("dep.qi");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["build"]).arg(&app_main).arg("--release");
    let output = run_command_capture(
        &mut command,
        "`ql build --release` direct project source file",
    );
    let (stdout, stderr) = expect_success(
        "project-build-source-file-release-override",
        "direct project source file release alias build",
        &output,
    )
    .expect("direct project source file `ql build --release` should succeed");
    expect_empty_stderr(
        "project-build-source-file-release-override",
        "direct project source file release alias build",
        &stderr,
    )
    .expect("direct project source file release alias build should not print stderr");
    expect_stdout_contains_all(
        "project-build-source-file-release-override",
        &stdout.replace('\\', "/"),
        &[
            &format!("wrote staticlib: {}", dep_stdout_output.display()).replace('\\', "/"),
            &format!("wrote llvm-ir: {}", app_output.display()).replace('\\', "/"),
        ],
    )
    .expect("direct project source file release alias build should report release artifacts");
    expect_file_exists(
        "project-build-source-file-release-override",
        &dep_output,
        "dependency package release artifact",
        "direct project source file release alias build",
    )
    .expect("direct project source file release alias build should emit dependency artifact");
    expect_file_exists(
        "project-build-source-file-release-override",
        &app_output,
        "selected package release artifact",
        "direct project source file release alias build",
    )
    .expect("direct project source file release alias build should emit selected artifact");
    expect_file_exists(
        "project-build-source-file-release-override",
        &interface_output,
        "synced dependency interface",
        "direct project source file release alias build",
    )
    .expect("direct project source file release alias build should keep dependency interface sync");
    assert!(
        !project_root.join("target/ql/debug/main.ll").exists(),
        "direct project source file release alias build should not emit debug main artifact"
    );
}

#[test]
fn build_project_path_rejects_output_for_multiple_targets() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-output");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create package source tree for output rejection test");
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write("app/src/lib.ql", "pub fn util() -> Int { return 1 }\n");
    temp.write("app/src/main.ql", "fn main() -> Int { return 0 }\n");

    let output_path = project_root.join("custom.ll");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["build"])
        .arg(&project_root)
        .args(["--output"])
        .arg(&output_path);
    let output = run_command_capture(&mut command, "`ql build --output` multiple targets");
    let (stdout, stderr) = expect_exit_code(
        "project-build-output",
        "multiple target output rejection",
        &output,
        1,
    )
    .expect("project build should reject `--output` when multiple targets are discovered");
    expect_empty_stdout(
        "project-build-output",
        "multiple target output rejection",
        &stdout,
    )
    .expect("output rejection should not print stdout");
    assert!(
        stderr
            .contains("error: `ql build --output` only supports a single discovered build target"),
        "expected multi-target output rejection, got:\n{stderr}"
    );
}

#[test]
fn build_package_path_selects_requested_target_for_custom_output() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-target-selector");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src/bin/tools"))
        .expect("create package source tree for target selector build test");
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write("app/src/lib.ql", "pub fn util() -> Int { return 1 }\n");
    temp.write("app/src/main.ql", "fn main() -> Int { return 0 }\n");
    temp.write(
        "app/src/bin/tools/repl.ql",
        "fn main() -> Int { return 2 }\n",
    );

    let output_path = project_root.join("custom.ll");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["build"])
        .arg(&project_root)
        .args(["--target", "src/bin/tools/repl.ql", "--output"])
        .arg(&output_path);
    let output = run_command_capture(&mut command, "`ql build --target --output` package path");
    let (stdout, stderr) = expect_success(
        "project-build-target-selector",
        "selected target custom output build",
        &output,
    )
    .expect("package-path `ql build --target --output` should succeed for one selected target");
    expect_empty_stderr(
        "project-build-target-selector",
        "selected target custom output build",
        &stderr,
    )
    .expect("package-path `ql build --target --output` should not print stderr");
    expect_stdout_contains_all(
        "project-build-target-selector",
        &stdout.replace('\\', "/"),
        &[&format!("wrote llvm-ir: {}", output_path.display()).replace('\\', "/")],
    )
    .expect("package-path `ql build --target --output` should report the selected artifact");
    expect_file_exists(
        "project-build-target-selector",
        &output_path,
        "selected target custom output artifact",
        "selected target custom output build",
    )
    .expect("package-path `ql build --target --output` should write the selected artifact");
}

#[test]
fn build_project_path_emits_interface_once_for_multiple_targets() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-emit-interface");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create package source tree for emit-interface build test");
    temp.write(
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

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["build"])
        .arg(&project_root)
        .arg("--emit-interface");
    let output = run_command_capture(&mut command, "`ql build --emit-interface` package path");
    let (stdout, stderr) = expect_success(
        "project-build-emit-interface",
        "package path build with interface emission",
        &output,
    )
    .expect("package path build with interface emission should succeed");
    expect_empty_stderr(
        "project-build-emit-interface",
        "package path build with interface emission",
        &stderr,
    )
    .expect("package path build with interface emission should not print stderr");
    expect_stdout_contains_all(
        "project-build-emit-interface",
        &stdout.replace('\\', "/"),
        &[
            &format!("wrote staticlib: {}", lib_output.display()).replace('\\', "/"),
            &format!("wrote llvm-ir: {}", main_output.display()).replace('\\', "/"),
            &format!("wrote interface: {}", interface_output.display()).replace('\\', "/"),
        ],
    )
    .expect("package path build with interface emission should report artifacts and interface");
    expect_file_exists(
        "project-build-emit-interface",
        &interface_output,
        "package interface artifact",
        "package path build with interface emission",
    )
    .expect("package path build with interface emission should write the package interface");
}

#[test]
fn build_package_path_syncs_dependency_interfaces_from_manifest_dependencies() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-dependency-sync");
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
        "use dep.q_add as add\n\nfn main() -> Int { return add(2, 3) }\n",
    );

    let interface_output = dep_root.join("dep.qi");
    let dep_output = static_library_output_path(&dep_root.join("target/ql/debug"), "lib");
    let dep_output_suffix = "dep/target/ql/debug/lib.lib";
    let app_output = project_root.join("target/ql/debug/main.ll");
    assert!(
        !interface_output.exists(),
        "dependency interface should start missing for sync test"
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["build"]).arg(&project_root);
    let output = run_command_capture(&mut command, "`ql build` dependency sync");
    let (stdout, stderr) = expect_success(
        "project-build-dependency-sync",
        "package path build with dependency sync",
        &output,
    )
    .expect("package-path `ql build` should sync dependency interfaces before building");
    expect_empty_stderr(
        "project-build-dependency-sync",
        "package path build with dependency sync",
        &stderr,
    )
    .expect("dependency-sync build should not print stderr");
    expect_stdout_contains_all(
        "project-build-dependency-sync",
        &stdout.replace('\\', "/"),
        &[
            "wrote interface: ",
            "dep.qi",
            dep_output_suffix,
            &format!("wrote llvm-ir: {}", app_output.display()).replace('\\', "/"),
        ],
    )
    .expect(
        "dependency-sync build should report the synced interface plus dependency and root artifacts",
    );
    let normalized_stdout = stdout.replace('\\', "/");
    let dep_fragment = dep_output_suffix.to_owned();
    let app_fragment = format!("wrote llvm-ir: {}", app_output.display()).replace('\\', "/");
    let dep_index = normalized_stdout
        .find(&dep_fragment)
        .expect("dependency artifact should be present in stdout");
    let app_index = normalized_stdout
        .find(&app_fragment)
        .expect("root artifact should be present in stdout");
    assert!(
        dep_index < app_index,
        "dependency package should be built before the root package, got:\n{stdout}"
    );
    expect_file_exists(
        "project-build-dependency-sync",
        &interface_output,
        "synced dependency interface",
        "package path build with dependency sync",
    )
    .expect("dependency-sync build should emit the dependency interface");
    expect_file_exists(
        "project-build-dependency-sync",
        &dep_output,
        "dependency package build artifact",
        "package path build with dependency sync",
    )
    .expect("dependency-sync build should emit the dependency package artifact");
    expect_file_exists(
        "project-build-dependency-sync",
        &app_output,
        "package build artifact",
        "package path build with dependency sync",
    )
    .expect("dependency-sync build should still emit the package artifact");
}
