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
