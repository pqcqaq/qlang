mod support;

use serde_json::Value as JsonValue;
use support::{
    TempDir, expect_empty_stderr, expect_empty_stdout, expect_exit_code, expect_stderr_contains,
    ql_command, run_command_capture, workspace_root,
};

fn normalize_output_text(text: &str) -> String {
    text.replace("\r\n", "\n")
}

fn parse_json_output(case_name: &str, stdout: &str) -> JsonValue {
    serde_json::from_str(&normalize_output_text(stdout))
        .unwrap_or_else(|error| panic!("[{case_name}] parse json stdout: {error}\n{stdout}"))
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
fn run_project_path_json_reports_missing_manifest_preflight_failure() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-json-missing-manifest");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(&project_root)
        .expect("create empty project directory for missing manifest run json test");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["run"]).arg(&project_root).arg("--json");
    let output = run_command_capture(&mut command, "`ql run --json` missing manifest");
    let (stdout, stderr) = expect_exit_code(
        "project-run-json-missing-manifest",
        "missing manifest run json preflight failure",
        &output,
        1,
    )
    .expect("project-path `ql run --json` should exit with code 1 for missing manifest");
    expect_empty_stderr(
        "project-run-json-missing-manifest",
        "missing manifest run json preflight failure",
        &stderr,
    )
    .expect("project-path `ql run --json` missing manifest should stay on stdout");

    let json = parse_json_output("project-run-json-missing-manifest", &stdout);
    assert_eq!(json["schema"], "ql.run.v1");
    assert_eq!(
        json["path"],
        project_root.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["scope"], "project");
    assert_eq!(json["project_manifest_path"], JsonValue::Null);
    assert_eq!(json["status"], "failed");
    assert_eq!(json["built_target"], JsonValue::Null);
    assert_eq!(json["execution"], JsonValue::Null);
    assert_eq!(json["failure"]["kind"], "preflight");
    let failure = &json["failure"]["preflight_failure"];
    assert_eq!(failure["error_kind"], "manifest");
    assert_eq!(failure["stage"], "manifest-load");
    assert_eq!(failure["target_count"], JsonValue::Null);
    assert!(
        failure["message"]
            .as_str()
            .expect("missing manifest run json failure should expose message")
            .contains("could not find `qlang.toml`"),
        "missing manifest run json failure should describe manifest lookup: {json}"
    );
}

#[test]
fn run_project_path_json_reports_missing_source_root_preflight_failure() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-json-missing-source-root");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(&project_root)
        .expect("create project directory for missing source root run json test");
    let app_manifest = temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["run"]).arg(&project_root).arg("--json");
    let output = run_command_capture(&mut command, "`ql run --json` missing source root");
    let (stdout, stderr) = expect_exit_code(
        "project-run-json-missing-source-root",
        "missing source root run json preflight failure",
        &output,
        1,
    )
    .expect("project-path `ql run --json` should exit with code 1 for missing source root");
    expect_empty_stderr(
        "project-run-json-missing-source-root",
        "missing source root run json preflight failure",
        &stderr,
    )
    .expect("project-path `ql run --json` missing source root should stay on stdout");

    let json = parse_json_output("project-run-json-missing-source-root", &stdout);
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
    assert_eq!(json["status"], "failed");
    assert_eq!(json["built_target"], JsonValue::Null);
    assert_eq!(json["execution"], JsonValue::Null);
    assert_eq!(json["failure"]["kind"], "preflight");
    let failure = &json["failure"]["preflight_failure"];
    assert_eq!(failure["error_kind"], "manifest");
    assert_eq!(failure["stage"], "target-discovery");
    assert_eq!(failure["target_count"], JsonValue::Null);
    assert!(
        failure["message"]
            .as_str()
            .expect("missing source root run json failure should expose message")
            .contains("package source directory"),
        "missing source root run json failure should describe source root lookup: {json}"
    );
}

#[test]
fn run_project_path_json_reports_no_runnable_targets_preflight_failure() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-json-no-runnable");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create package source tree for no-runnable run json test");
    let app_manifest = temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write("app/src/lib.ql", "pub fn helper() -> Int { return 1 }\n");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["run"]).arg(&project_root).arg("--json");
    let output = run_command_capture(&mut command, "`ql run --json` no runnable targets");
    let (stdout, stderr) = expect_exit_code(
        "project-run-json-no-runnable",
        "no runnable targets run json preflight failure",
        &output,
        1,
    )
    .expect("project-path `ql run --json` should exit with code 1 without runnable targets");
    expect_empty_stderr(
        "project-run-json-no-runnable",
        "no runnable targets run json preflight failure",
        &stderr,
    )
    .expect("project-path `ql run --json` no-runnable failure should stay on stdout");

    let json = parse_json_output("project-run-json-no-runnable", &stdout);
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
    assert_eq!(json["status"], "failed");
    assert_eq!(json["built_target"], JsonValue::Null);
    assert_eq!(json["execution"], JsonValue::Null);
    assert_eq!(json["failure"]["kind"], "preflight");
    let failure = &json["failure"]["preflight_failure"];
    assert_eq!(failure["error_kind"], "project");
    assert_eq!(failure["stage"], "target-selection");
    assert_eq!(failure["selector"], JsonValue::Null);
    assert_eq!(failure["target_count"], 0);
    assert!(
        failure["message"]
            .as_str()
            .expect("no-runnable run json failure should expose message")
            .contains("found no runnable build targets"),
        "no-runnable run json failure should describe target selection: {json}"
    );
}

#[test]
fn run_project_path_json_reports_multiple_runnable_targets_preflight_failure() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-json-multiple-runnable");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src/bin"))
        .expect("create package source tree for multiple-runnable run json test");
    let app_manifest = temp.write(
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
    command.args(["run"]).arg(&project_root).arg("--json");
    let output = run_command_capture(&mut command, "`ql run --json` multiple runnable targets");
    let (stdout, stderr) = expect_exit_code(
        "project-run-json-multiple-runnable",
        "multiple runnable targets run json preflight failure",
        &output,
        1,
    )
    .expect("project-path `ql run --json` should exit with code 1 for multiple runnable targets");
    expect_empty_stderr(
        "project-run-json-multiple-runnable",
        "multiple runnable targets run json preflight failure",
        &stderr,
    )
    .expect("project-path `ql run --json` multiple-runnable failure should stay on stdout");

    let json = parse_json_output("project-run-json-multiple-runnable", &stdout);
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
    assert_eq!(json["status"], "failed");
    assert_eq!(json["built_target"], JsonValue::Null);
    assert_eq!(json["execution"], JsonValue::Null);
    assert_eq!(json["failure"]["kind"], "preflight");
    let failure = &json["failure"]["preflight_failure"];
    assert_eq!(failure["error_kind"], "project");
    assert_eq!(failure["stage"], "target-selection");
    assert_eq!(failure["selector"], JsonValue::Null);
    assert_eq!(failure["target_count"], 2);
    assert!(
        failure["message"]
            .as_str()
            .expect("multiple-runnable run json failure should expose message")
            .contains("found multiple runnable build targets"),
        "multiple-runnable run json failure should describe target selection: {json}"
    );
}

#[test]
fn run_single_file_json_rejects_target_selectors_without_project_context() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-file-json-selector-context");
    let source_path = temp.write("sample.ql", "fn main() -> Int { return 0 }\n");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["run"])
        .arg(&source_path)
        .args(["--json", "--bin", "admin"]);
    let output = run_command_capture(
        &mut command,
        "`ql run --json` single file selector requires project context",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-run-file-json-selector-context",
        "single-file run json selector preflight failure",
        &output,
        1,
    )
    .expect("single-file `ql run --json --bin admin` should exit with code 1");
    expect_empty_stderr(
        "project-run-file-json-selector-context",
        "single-file run json selector preflight failure",
        &stderr,
    )
    .expect("single-file `ql run --json --bin admin` should not print stderr");

    let json = parse_json_output("project-run-file-json-selector-context", &stdout);
    assert_eq!(json["schema"], "ql.run.v1");
    assert_eq!(json["scope"], "file");
    assert_eq!(json["project_manifest_path"], JsonValue::Null);
    assert_eq!(json["status"], "failed");
    assert_eq!(json["built_target"], JsonValue::Null);
    assert_eq!(json["execution"], JsonValue::Null);
    assert_eq!(json["failure"]["kind"], "preflight");
    let failure = &json["failure"]["preflight_failure"];
    assert_eq!(failure["error_kind"], "selector");
    assert_eq!(failure["stage"], "project-context");
    assert_eq!(failure["selector"], "binary `admin`");
    assert_eq!(
        failure["message"],
        "target selectors require a package or workspace path"
    );
}

#[test]
fn run_project_source_path_json_rejects_explicit_target_selector() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-source-json-selector-conflict");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src/bin"))
        .expect("create package source tree for source selector conflict run json test");
    let app_manifest = temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    let main_path = temp.write("app/src/main.ql", "fn main() -> Int { return 1 }\n");
    temp.write("app/src/bin/admin.ql", "fn main() -> Int { return 2 }\n");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["run"])
        .arg(&main_path)
        .args(["--json", "--bin", "admin"]);
    let output = run_command_capture(
        &mut command,
        "`ql run --json` project source selector conflict",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-run-source-json-selector-conflict",
        "project source run json selector preflight failure",
        &output,
        1,
    )
    .expect("project source `ql run --json --bin admin` should exit with code 1");
    expect_empty_stderr(
        "project-run-source-json-selector-conflict",
        "project source run json selector preflight failure",
        &stderr,
    )
    .expect("project source `ql run --json --bin admin` should not print stderr");

    let json = parse_json_output("project-run-source-json-selector-conflict", &stdout);
    assert_eq!(json["schema"], "ql.run.v1");
    assert_eq!(json["scope"], "project");
    assert_eq!(
        json["project_manifest_path"],
        app_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(json["status"], "failed");
    assert_eq!(json["built_target"], JsonValue::Null);
    assert_eq!(json["execution"], JsonValue::Null);
    assert_eq!(json["failure"]["kind"], "preflight");
    let failure = &json["failure"]["preflight_failure"];
    assert_eq!(failure["error_kind"], "selector");
    assert_eq!(failure["stage"], "project-context");
    assert_eq!(failure["selector"], "binary `admin`");
    assert_eq!(
        failure["message"],
        "direct project source paths do not support target selectors"
    );
}

#[test]
fn run_project_list_json_reports_selected_non_runnable_target() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-list-json-selected-non-runnable");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src")).expect("create package source root");
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
    command
        .args(["run", "--list", "--json", "--lib"])
        .arg(&project_root);
    let output = run_command_capture(&mut command, "`ql run --list --json --lib` package path");
    let (stdout, stderr) = expect_exit_code(
        "project-run-list-json-selected-non-runnable",
        "selected non-runnable target run list json",
        &output,
        1,
    )
    .expect("package-path `ql run --list --json --lib` should fail with no runnable targets");
    expect_empty_stderr(
        "project-run-list-json-selected-non-runnable",
        "selected non-runnable target run list json",
        &stderr,
    )
    .expect("selected non-runnable target run list json should stay on stdout");

    let json = parse_json_output("project-run-list-json-selected-non-runnable", &stdout);
    assert_eq!(json["schema"], "ql.project.targets.v1");
    assert_eq!(json["members"], serde_json::json!([]));
    assert_eq!(json["failure"]["kind"], "selection");
    let failure = &json["failure"]["selection_failure"];
    assert_eq!(failure["stage"], "runnable-selection");
    assert_eq!(failure["selector"], "library target");
    assert_eq!(failure["target_count"], 1);
    assert!(
        failure["message"]
            .as_str()
            .expect("run list json runnable selection failure should expose a message")
            .contains("target selector matched no runnable build targets"),
        "selected non-runnable target run list json should describe runnable selection: {json}"
    );
}

#[test]
fn run_project_list_json_reports_selector_miss_on_stdout() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-list-json-selector-miss");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src")).expect("create package source root");
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write("app/src/lib.ql", "pub fn helper() -> Int { return 1 }\n");
    temp.write("app/src/main.ql", "fn main() -> Int { return 0 }\n");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["run", "--list", "--json", "--bin", "missing"])
        .arg(&project_root);
    let output = run_command_capture(&mut command, "`ql run --list --json --bin` selector miss");
    let (stdout, stderr) = expect_exit_code(
        "project-run-list-json-selector-miss",
        "run list json selector miss",
        &output,
        1,
    )
    .expect("package-path `ql run --list --json --bin missing` should fail");
    expect_empty_stderr(
        "project-run-list-json-selector-miss",
        "run list json selector miss",
        &stderr,
    )
    .expect("run list json selector miss should stay on stdout");

    let json = parse_json_output("project-run-list-json-selector-miss", &stdout);
    assert_eq!(json["schema"], "ql.project.targets.v1");
    assert_eq!(json["members"], serde_json::json!([]));
    assert_eq!(json["failure"]["kind"], "selection");
    let failure = &json["failure"]["selection_failure"];
    assert_eq!(failure["stage"], "target-selection");
    assert_eq!(failure["selector"], "binary `missing`");
    assert_eq!(failure["target_count"], 2);
    assert!(
        failure["message"]
            .as_str()
            .expect("run list json selector miss should expose a message")
            .contains("target selector matched no build targets"),
        "run list json selector miss should describe target selection: {json}"
    );
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
fn run_project_source_path_rejects_explicit_target_selector() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-source-selector-conflict");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src/bin"))
        .expect("create package source tree for source selector conflict run test");
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    let main_path = temp.write("app/src/main.ql", "fn main() -> Int { return 1 }\n");
    temp.write("app/src/bin/admin.ql", "fn main() -> Int { return 2 }\n");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["run"])
        .arg(&main_path)
        .args(["--bin", "admin"]);
    let output = run_command_capture(
        &mut command,
        "`ql run` project source path explicit selector conflict",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-run-source-selector-conflict",
        "project source selector conflict",
        &output,
        1,
    )
    .expect("project source path `ql run --bin admin` should exit with code 1");
    expect_empty_stdout(
        "project-run-source-selector-conflict",
        "project source selector conflict",
        &stdout,
    )
    .expect("project source selector conflict should not print stdout");
    expect_stderr_contains(
        "project-run-source-selector-conflict",
        "project source selector conflict",
        &stderr,
        "error: `ql run` does not support combining a direct project source path with target selectors",
    )
    .expect("project source selector conflict should explain the invalid combination");
    expect_stderr_contains(
        "project-run-source-selector-conflict",
        "project source selector conflict",
        &stderr,
        "note: selector: binary `admin`",
    )
    .expect("project source selector conflict should print the selector note");
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
