mod support;

use support::{
    TempDir, expect_empty_stderr, expect_empty_stdout, expect_exit_code, expect_snapshot_matches,
    expect_stderr_contains, expect_stderr_not_contains, expect_stdout_contains_all, expect_success,
    ql_command, run_command_capture, workspace_root,
};

#[test]
fn project_graph_prints_package_workspace_and_references() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-graph");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create nested project directory for project graph test");
    std::fs::create_dir_all(temp.path().join("workspace").join("core"))
        .expect("create core directory for project graph test");
    std::fs::create_dir_all(temp.path().join("workspace").join("runtime"))
        .expect("create runtime directory for project graph test");
    let manifest_path = temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[workspace]
members = ["packages/app", "packages/core"]

[references]
packages = ["../core", "../runtime"]
"#,
    );
    temp.write(
        "workspace/core/qlang.toml",
        r#"
[package]
name = "core"
"#,
    );
    temp.write(
        "workspace/runtime/qlang.toml",
        r#"
[package]
name = "runtime"
"#,
    );
    temp.write("workspace/app/app.qi", "broken interface\n");
    temp.write(
        "workspace/runtime/runtime.qi",
        r#"
// qlang interface v1
// package: runtime

// source: src/lib.ql
package demo.runtime

pub fn run() -> Int
"#,
    );

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "graph"])
        .arg(project_root.join("src"));
    let output = run_command_capture(&mut command, "`ql project graph`");
    let (stdout, stderr) =
        expect_success("project-graph-success", "project graph rendering", &output)
            .expect("project graph rendering should succeed");
    expect_empty_stderr("project-graph-success", "project graph rendering", &stderr)
        .expect("successful project graph rendering should stay silent on stderr");

    let expected = format!(
        "manifest: {}\npackage: app\nworkspace_members:\n  - packages/app\n  - packages/core\nreferences:\n  - ../core\n  - ../runtime\ninterface:\n  path: app.qi\n  status: invalid\n  detail: expected `// qlang interface v1` header\nreference_interfaces:\n  - reference: ../core\n    manifest: ../core/qlang.toml\n    package: core\n    path: ../core/core.qi\n    status: missing\n  - reference: ../runtime\n    manifest: ../runtime/qlang.toml\n    package: runtime\n    path: ../runtime/runtime.qi\n    status: valid\n",
        manifest_path.to_string_lossy().replace('\\', "/")
    );
    expect_snapshot_matches(
        "project-graph-success",
        "project graph stdout",
        &expected,
        &stdout,
    )
    .expect("project graph stdout should match the resolved manifest graph");
}

#[test]
fn project_graph_supports_json_output_for_package_graph() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-graph-json");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create nested project directory for project graph json test");
    std::fs::create_dir_all(temp.path().join("workspace").join("core"))
        .expect("create core directory for project graph json test");
    std::fs::create_dir_all(temp.path().join("workspace").join("runtime"))
        .expect("create runtime directory for project graph json test");
    let manifest_path = temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[workspace]
members = ["packages/app", "packages/core"]

[references]
packages = ["../core", "../runtime"]
"#,
    );
    temp.write(
        "workspace/core/qlang.toml",
        r#"
[package]
name = "core"
"#,
    );
    temp.write(
        "workspace/runtime/qlang.toml",
        r#"
[package]
name = "runtime"
"#,
    );
    temp.write("workspace/app/app.qi", "broken interface\n");
    temp.write(
        "workspace/runtime/runtime.qi",
        r#"
// qlang interface v1
// package: runtime

// source: src/lib.ql
package demo.runtime

pub fn run() -> Int
"#,
    );

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "graph", "--json"])
        .arg(project_root.join("src"));
    let output = run_command_capture(&mut command, "`ql project graph --json`");
    let (stdout, stderr) = expect_success(
        "project-graph-json-success",
        "project graph json rendering",
        &output,
    )
    .expect("project graph json rendering should succeed");
    expect_empty_stderr(
        "project-graph-json-success",
        "project graph json rendering",
        &stderr,
    )
    .expect("successful project graph json rendering should stay silent on stderr");

    let expected = format!(
        "{{\n  \"interface\": {{\n    \"detail\": \"expected `// qlang interface v1` header\",\n    \"path\": \"app.qi\",\n    \"stale_reasons\": [],\n    \"status\": \"invalid\"\n  }},\n  \"manifest_path\": \"{}\",\n  \"package_name\": \"app\",\n  \"reference_interfaces\": [\n    {{\n      \"detail\": null,\n      \"manifest_path\": \"../core/qlang.toml\",\n      \"package_name\": \"core\",\n      \"path\": \"../core/core.qi\",\n      \"reference\": \"../core\",\n      \"stale_reasons\": [],\n      \"status\": \"missing\",\n      \"transitive_reference_failures\": {{\n        \"count\": 0,\n        \"first_failure\": null\n      }}\n    }},\n    {{\n      \"detail\": null,\n      \"manifest_path\": \"../runtime/qlang.toml\",\n      \"package_name\": \"runtime\",\n      \"path\": \"../runtime/runtime.qi\",\n      \"reference\": \"../runtime\",\n      \"stale_reasons\": [],\n      \"status\": \"valid\",\n      \"transitive_reference_failures\": {{\n        \"count\": 0,\n        \"first_failure\": null\n      }}\n    }}\n  ],\n  \"references\": [\n    \"../core\",\n    \"../runtime\"\n  ],\n  \"schema\": \"ql.project.graph.v1\",\n  \"workspace_members\": [\n    \"packages/app\",\n    \"packages/core\"\n  ],\n  \"workspace_packages\": []\n}}\n",
        manifest_path.to_string_lossy().replace('\\', "/")
    );
    expect_snapshot_matches(
        "project-graph-json-success",
        "project graph json stdout",
        &expected,
        &stdout.replace('\\', "/"),
    )
    .expect("project graph json stdout should match the resolved manifest graph");
}

#[test]
fn project_graph_rejects_manifest_without_package_or_workspace() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-graph-invalid");
    let project_root = temp.path().join("invalid");
    std::fs::create_dir_all(&project_root).expect("create invalid project directory");
    temp.write(
        "invalid/qlang.toml",
        r#"
[references]
packages = ["../core"]
"#,
    );

    let mut command = ql_command(&workspace_root);
    command.args(["project", "graph"]).arg(&project_root);
    let output = run_command_capture(&mut command, "`ql project graph` invalid manifest");
    let (stdout, stderr) = expect_exit_code(
        "project-graph-invalid",
        "invalid project graph rendering",
        &output,
        1,
    )
    .expect("invalid project graph rendering should fail with exit code 1");
    expect_empty_stdout(
        "project-graph-invalid",
        "invalid project graph rendering",
        &stdout,
    )
    .expect("invalid project graph rendering should not print stdout");
    expect_stderr_contains(
        "project-graph-invalid",
        "invalid project graph rendering",
        &stderr,
        "`qlang.toml` requires `[package]` or `[workspace]`",
    )
    .expect("invalid manifest diagnostic should mention the minimum section contract");
}

#[test]
fn project_graph_points_to_missing_package_context() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-graph-missing-package-context");
    let source_path = temp.write(
        "workspace/loose.ql",
        r#"
fn main() -> Int {
    return 1
}
"#,
    );

    let mut command = ql_command(&workspace_root);
    command.args(["project", "graph"]).arg(&source_path);
    let output = run_command_capture(&mut command, "`ql project graph` missing package context");
    let (stdout, stderr) = expect_exit_code(
        "project-graph-missing-package-context",
        "project graph rendering with missing package context",
        &output,
        1,
    )
    .expect("project graph should fail when the target path is outside any package/workspace");
    expect_empty_stdout(
        "project-graph-missing-package-context",
        "project graph rendering with missing package context",
        &stdout,
    )
    .expect("project graph should not print stdout when package context is missing");
    let normalized_stderr = stderr.replace('\\', "/");
    let source_display = source_path.to_string_lossy().replace('\\', "/");
    let error_line = format!(
        "error: `ql project graph` requires a package or workspace manifest; could not find `qlang.toml` starting from `{source_display}`"
    );
    let old_error_line =
        format!("error: could not find `qlang.toml` starting from `{source_display}`");
    let rerun_hint = format!(
        "hint: rerun `ql project graph {source_display}` after adding `qlang.toml` for this path"
    );
    expect_stderr_contains(
        "project-graph-missing-package-context",
        "project graph rendering with missing package context",
        &normalized_stderr,
        &error_line,
    )
    .expect("project graph should preserve the command label for missing package context");
    expect_stderr_not_contains(
        "project-graph-missing-package-context",
        "project graph rendering with missing package context",
        &normalized_stderr,
        &old_error_line,
    )
    .expect("project graph should not fall back to the unlabeled manifest-not-found error");
    expect_stderr_contains(
        "project-graph-missing-package-context",
        "project graph rendering with missing package context",
        &normalized_stderr,
        "note: `ql project graph` only renders package/workspace graphs for packages or workspace members discoverable from `qlang.toml`",
    )
    .expect("project graph should explain the package/workspace discovery contract");
    expect_stderr_contains(
        "project-graph-missing-package-context",
        "project graph rendering with missing package context",
        &normalized_stderr,
        &rerun_hint,
    )
    .expect("project graph should preserve the original target path in the rerun hint");
    expect_stderr_not_contains(
        "project-graph-missing-package-context",
        "project graph rendering with missing package context",
        &normalized_stderr,
        "note: failing package manifest:",
    )
    .expect("project graph should not pretend a package manifest was already found");
}

#[test]
fn project_graph_preserves_invalid_manifest_rerun_hint() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-graph-invalid-manifest");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(&project_root)
        .expect("create project directory for invalid manifest graph test");
    let manifest_path = temp.write(
        "workspace/app/qlang.toml",
        r#"
[package
name = "app"
"#,
    );

    let mut command = ql_command(&workspace_root);
    command.args(["project", "graph"]).arg(&project_root);
    let output = run_command_capture(&mut command, "`ql project graph` invalid manifest");
    let (stdout, stderr) = expect_exit_code(
        "project-graph-invalid-manifest",
        "project graph rendering with invalid manifest",
        &output,
        1,
    )
    .expect("project graph should fail when the manifest is syntactically invalid");
    expect_empty_stdout(
        "project-graph-invalid-manifest",
        "project graph rendering with invalid manifest",
        &stdout,
    )
    .expect("project graph should not print stdout for invalid manifests");
    let normalized_stderr = stderr.replace('\\', "/");
    let manifest_display = manifest_path.to_string_lossy().replace('\\', "/");
    let error_line = format!("error: `ql project graph` invalid manifest `{manifest_display}`");
    let old_error_line = format!("error: invalid manifest `{manifest_display}`");
    let manifest_note = format!("note: failing package manifest: {manifest_display}");
    let rerun_hint = format!(
        "hint: rerun `ql project graph {manifest_display}` after fixing the package manifest"
    );
    expect_stderr_contains(
        "project-graph-invalid-manifest",
        "project graph rendering with invalid manifest",
        &normalized_stderr,
        &error_line,
    )
    .expect("project graph should preserve the command label for invalid manifests");
    expect_stderr_not_contains(
        "project-graph-invalid-manifest",
        "project graph rendering with invalid manifest",
        &normalized_stderr,
        &old_error_line,
    )
    .expect("project graph should not fall back to the unlabeled invalid-manifest error");
    expect_stderr_contains(
        "project-graph-invalid-manifest",
        "project graph rendering with invalid manifest",
        &normalized_stderr,
        &manifest_note,
    )
    .expect("project graph should point to the failing package manifest");
    expect_stderr_contains(
        "project-graph-invalid-manifest",
        "project graph rendering with invalid manifest",
        &normalized_stderr,
        &rerun_hint,
    )
    .expect("project graph should preserve the direct rerun hint for invalid manifests");
}

#[test]
fn project_graph_preserves_missing_package_name_error_surface() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-graph-missing-package-name");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(&project_root)
        .expect("create project directory for missing package name graph test");
    let manifest_path = temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
version = "0.1.0"
"#,
    );

    let mut command = ql_command(&workspace_root);
    command.args(["project", "graph"]).arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql project graph` missing package manifest metadata",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-graph-missing-package-name",
        "project graph rendering with missing package name",
        &output,
        1,
    )
    .expect("project graph should fail when the manifest does not declare `[package].name`");
    expect_empty_stdout(
        "project-graph-missing-package-name",
        "project graph rendering with missing package name",
        &stdout,
    )
    .expect("project graph should not print stdout when package metadata is missing");
    let normalized_stderr = stderr.replace('\\', "/");
    let manifest_display = manifest_path.to_string_lossy().replace('\\', "/");
    expect_stderr_contains(
        "project-graph-missing-package-name",
        "project graph rendering with missing package name",
        &normalized_stderr,
        &format!(
            "error: `ql project graph` manifest `{manifest_display}` does not declare `[package].name`"
        ),
    )
    .expect("project graph should preserve the command label for missing package names");
    expect_stderr_not_contains(
        "project-graph-missing-package-name",
        "project graph rendering with missing package name",
        &normalized_stderr,
        &format!("error: invalid manifest `{manifest_display}`: `[package].name` must be present"),
    )
    .expect("project graph should not fall back to the parse-error missing package-name message");
    expect_stderr_contains(
        "project-graph-missing-package-name",
        "project graph rendering with missing package name",
        &normalized_stderr,
        &format!("note: failing package manifest: {manifest_display}"),
    )
    .expect("project graph should point to the failing package manifest");
    expect_stderr_contains(
        "project-graph-missing-package-name",
        "project graph rendering with missing package name",
        &normalized_stderr,
        &format!(
            "hint: rerun `ql project graph {manifest_display}` after fixing the package manifest"
        ),
    )
    .expect("project graph should preserve the direct rerun hint for missing package names");
}

#[test]
fn project_graph_reports_stale_package_and_reference_interfaces() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-graph-stale-interface");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create app source directory for stale interface test");
    std::fs::create_dir_all(temp.path().join("workspace").join("dep").join("src"))
        .expect("create dep source directory for stale interface test");
    let manifest_path = temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "workspace/app/src/lib.ql",
        r#"
pub fn run() -> Int {
    1
}
"#,
    );
    temp.write(
        "workspace/dep/src/lib.ql",
        r#"
pub fn exported() -> Int {
    1
}
"#,
    );
    temp.write(
        "workspace/app/app.qi",
        r#"
// qlang interface v1
// package: app

// source: src/lib.ql
package demo.app

pub fn run() -> Int
"#,
    );
    temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub fn exported() -> Int
"#,
    );

    std::thread::sleep(std::time::Duration::from_millis(1200));

    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    temp.write(
        "workspace/dep/src/lib.ql",
        r#"
pub fn exported() -> Int {
    2
}
"#,
    );

    let mut command = ql_command(&workspace_root);
    command.args(["project", "graph"]).arg(&project_root);
    let output = run_command_capture(&mut command, "`ql project graph` stale interfaces");
    let (stdout, stderr) = expect_success(
        "project-graph-stale-interface",
        "project graph rendering with stale interfaces",
        &output,
    )
    .expect("project graph rendering with stale interfaces should succeed");
    expect_empty_stderr(
        "project-graph-stale-interface",
        "project graph rendering with stale interfaces",
        &stderr,
    )
    .expect("stale interface graph rendering should stay silent on stderr");

    let expected = format!(
        "manifest: {}\npackage: app\nworkspace_members: []\nreferences:\n  - ../dep\ninterface:\n  path: app.qi\n  status: stale\n  stale_reasons:\n    - manifest: qlang.toml\nreference_interfaces:\n  - reference: ../dep\n    manifest: ../dep/qlang.toml\n    package: dep\n    path: ../dep/dep.qi\n    status: stale\n    stale_reasons:\n      - source: ../dep/src/lib.ql\n",
        manifest_path.to_string_lossy().replace('\\', "/")
    );
    expect_snapshot_matches(
        "project-graph-stale-interface",
        "stale interface project graph stdout",
        &expected,
        &stdout,
    )
    .expect("project graph should report stale package and reference interfaces");
}

#[test]
fn project_graph_expands_workspace_root_members() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-graph-workspace-root");
    let project_root = temp.path().join("workspace");
    std::fs::create_dir_all(project_root.join("packages").join("app").join("src"))
        .expect("create workspace app directory");
    std::fs::create_dir_all(project_root.join("packages").join("tool").join("src"))
        .expect("create workspace tool directory");
    std::fs::create_dir_all(project_root.join("dep")).expect("create dependency directory");

    let manifest_path = temp.write(
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

[references]
packages = ["../../dep"]
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
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "workspace/packages/app/app.qi",
        r#"
// qlang interface v1
// package: app

// source: src/lib.ql
package demo.app

pub fn run() -> Int
"#,
    );
    temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub fn exported() -> Int
"#,
    );

    let mut command = ql_command(&workspace_root);
    command.args(["project", "graph"]).arg(&project_root);
    let output = run_command_capture(&mut command, "`ql project graph` workspace root");
    let (stdout, stderr) = expect_success(
        "project-graph-workspace-root",
        "workspace root project graph rendering",
        &output,
    )
    .expect("workspace root project graph rendering should succeed");
    expect_empty_stderr(
        "project-graph-workspace-root",
        "workspace root project graph rendering",
        &stderr,
    )
    .expect("workspace root project graph rendering should stay silent on stderr");

    let expected = format!(
        "manifest: {}\npackage: <none>\nworkspace_members:\n  - packages/app\n  - packages/tool\nreferences: []\nworkspace_packages:\n  - member: packages/app\n    manifest: packages/app/qlang.toml\n    package: app\n    interface:\n      path: packages/app/app.qi\n      status: valid\n    references:\n      - ../../dep\n    reference_interfaces:\n      - reference: ../../dep\n        manifest: dep/qlang.toml\n        package: dep\n        path: dep/dep.qi\n        status: valid\n  - member: packages/tool\n    manifest: packages/tool/qlang.toml\n    package: tool\n    interface:\n      path: packages/tool/tool.qi\n      status: missing\n    references: []\n    reference_interfaces: []\n",
        manifest_path.to_string_lossy().replace('\\', "/")
    );
    expect_snapshot_matches(
        "project-graph-workspace-root",
        "workspace root project graph stdout",
        &expected,
        &stdout,
    )
    .expect("workspace root project graph stdout should match resolved member graph");
}

#[test]
fn project_graph_supports_json_output_for_workspace_root() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-graph-workspace-root-json");
    let project_root = temp.path().join("workspace");
    std::fs::create_dir_all(project_root.join("packages").join("app").join("src"))
        .expect("create workspace app source tree for workspace root json test");
    std::fs::create_dir_all(project_root.join("packages").join("tool").join("src"))
        .expect("create workspace tool source tree for workspace root json test");
    std::fs::create_dir_all(project_root.join("dep"))
        .expect("create workspace dependency tree for workspace root json test");

    let manifest_path = temp.write(
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

[references]
packages = ["../../dep"]
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
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "workspace/packages/app/app.qi",
        r#"
// qlang interface v1
// package: app

// source: src/lib.ql
package demo.app

pub fn run() -> Int
"#,
    );
    temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub fn exported() -> Int
"#,
    );

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "graph"])
        .arg(&project_root)
        .arg("--json");
    let output = run_command_capture(&mut command, "`ql project graph --json` workspace root");
    let (stdout, stderr) = expect_success(
        "project-graph-workspace-root-json",
        "workspace root project graph json rendering",
        &output,
    )
    .expect("workspace root project graph json rendering should succeed");
    expect_empty_stderr(
        "project-graph-workspace-root-json",
        "workspace root project graph json rendering",
        &stderr,
    )
    .expect("workspace root project graph json rendering should stay silent on stderr");

    let expected = format!(
        "{{\n  \"interface\": null,\n  \"manifest_path\": \"{}\",\n  \"package_name\": null,\n  \"reference_interfaces\": [],\n  \"references\": [],\n  \"schema\": \"ql.project.graph.v1\",\n  \"workspace_members\": [\n    \"packages/app\",\n    \"packages/tool\"\n  ],\n  \"workspace_packages\": [\n    {{\n      \"interface\": {{\n        \"detail\": null,\n        \"path\": \"packages/app/app.qi\",\n        \"stale_reasons\": [],\n        \"status\": \"valid\"\n      }},\n      \"manifest_path\": \"packages/app/qlang.toml\",\n      \"member\": \"packages/app\",\n      \"member_error\": null,\n      \"member_status\": null,\n      \"package_name\": \"app\",\n      \"reference_interfaces\": [\n        {{\n          \"detail\": null,\n          \"manifest_path\": \"dep/qlang.toml\",\n          \"package_name\": \"dep\",\n          \"path\": \"dep/dep.qi\",\n          \"reference\": \"../../dep\",\n          \"stale_reasons\": [],\n          \"status\": \"valid\",\n          \"transitive_reference_failures\": {{\n            \"count\": 0,\n            \"first_failure\": null\n          }}\n        }}\n      ],\n      \"references\": [\n        \"../../dep\"\n      ]\n    }},\n    {{\n      \"interface\": {{\n        \"detail\": null,\n        \"path\": \"packages/tool/tool.qi\",\n        \"stale_reasons\": [],\n        \"status\": \"missing\"\n      }},\n      \"manifest_path\": \"packages/tool/qlang.toml\",\n      \"member\": \"packages/tool\",\n      \"member_error\": null,\n      \"member_status\": null,\n      \"package_name\": \"tool\",\n      \"reference_interfaces\": [],\n      \"references\": []\n    }}\n  ]\n}}\n",
        manifest_path.to_string_lossy().replace('\\', "/")
    );
    expect_snapshot_matches(
        "project-graph-workspace-root-json",
        "workspace root project graph json stdout",
        &expected,
        &stdout.replace('\\', "/"),
    )
    .expect("workspace root project graph json stdout should match resolved member graph");
}

#[test]
fn project_graph_keeps_resolved_workspace_members_when_one_member_manifest_is_invalid() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-graph-workspace-invalid-member");
    let project_root = temp.path().join("workspace");
    std::fs::create_dir_all(project_root.join("packages").join("app").join("src"))
        .expect("create workspace app directory for invalid member graph test");
    std::fs::create_dir_all(project_root.join("packages").join("broken"))
        .expect("create workspace broken directory for invalid member graph test");

    let manifest_path = temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/broken"]
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
        "workspace/packages/app/app.qi",
        r#"
// qlang interface v1
// package: app

// source: src/lib.ql
package demo.app

pub fn run() -> Int
"#,
    );
    temp.write(
        "workspace/packages/broken/qlang.toml",
        r#"
[package
name = "broken"
"#,
    );

    let mut command = ql_command(&workspace_root);
    command.args(["project", "graph"]).arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql project graph` workspace root with invalid member manifest",
    );
    let (stdout, stderr) = expect_success(
        "project-graph-workspace-invalid-member",
        "workspace root project graph rendering with invalid member manifest",
        &output,
    )
    .expect("workspace root graph should still render when one member manifest is invalid");
    expect_empty_stderr(
        "project-graph-workspace-invalid-member",
        "workspace root project graph rendering with invalid member manifest",
        &stderr,
    )
    .expect("workspace root graph with invalid member manifest should stay silent on stderr");

    let normalized_manifest = manifest_path.to_string_lossy().replace('\\', "/");
    let expected_prefix = format!(
        "manifest: {normalized_manifest}\npackage: <none>\nworkspace_members:\n  - packages/app\n  - packages/broken\nreferences: []\nworkspace_packages:\n  - member: packages/app\n    manifest: packages/app/qlang.toml\n    package: app\n    interface:\n      path: packages/app/app.qi\n      status: valid\n    references: []\n    reference_interfaces: []\n  - member: packages/broken\n    manifest: packages/broken/qlang.toml\n    package: <unresolved>\n    member_status: unresolved-manifest\n    member_error: invalid manifest `"
    );
    assert!(
        stdout.replace('\\', "/").starts_with(&expected_prefix),
        "expected workspace graph to keep resolved members and surface invalid member error, got:\n{stdout}"
    );
    assert!(
        stdout.contains("packages/broken/qlang.toml")
            || stdout.contains("packages\\broken\\qlang.toml"),
        "expected workspace graph to mention the broken member manifest path, got:\n{stdout}"
    );
}

#[test]
fn project_graph_keeps_resolved_workspace_members_when_one_member_manifest_is_missing_package_name()
{
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-graph-workspace-missing-package-name-member");
    let project_root = temp.path().join("workspace");
    std::fs::create_dir_all(project_root.join("packages").join("app").join("src"))
        .expect("create workspace app directory for missing package name member graph test");
    std::fs::create_dir_all(project_root.join("packages").join("broken"))
        .expect("create workspace broken directory for missing package name member graph test");

    let manifest_path = temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/broken"]
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
        "workspace/packages/app/app.qi",
        r#"
// qlang interface v1
// package: app

// source: src/lib.ql
package demo.app

pub fn run() -> Int
"#,
    );
    temp.write(
        "workspace/packages/broken/qlang.toml",
        r#"
[package]
version = "0.1.0"
"#,
    );

    let mut command = ql_command(&workspace_root);
    command.args(["project", "graph"]).arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql project graph` workspace root with missing package-name member",
    );
    let (stdout, stderr) = expect_success(
        "project-graph-workspace-missing-package-name-member",
        "workspace root project graph rendering with missing package name member",
        &output,
    )
    .expect(
        "workspace root graph should still render when one member manifest is missing `[package].name`",
    );
    expect_empty_stderr(
        "project-graph-workspace-missing-package-name-member",
        "workspace root project graph rendering with missing package name member",
        &stderr,
    )
    .expect("workspace root graph with missing package name member should stay silent on stderr");

    let normalized_stdout = stdout.replace('\\', "/");
    let normalized_manifest = manifest_path.to_string_lossy().replace('\\', "/");
    expect_stdout_contains_all(
        "project-graph-workspace-missing-package-name-member",
        &normalized_stdout,
        &[
            &format!("manifest: {normalized_manifest}"),
            "  - member: packages/app",
            "    package: app",
            "  - member: packages/broken",
            "    package: <unresolved>",
            "    member_status: unresolved-package",
            "    member_error: manifest `packages/broken/qlang.toml` does not declare `[package].name`",
        ],
    )
    .expect(
        "workspace graph should keep resolved members and normalize missing package-name member errors",
    );
    assert!(
        !normalized_stdout.contains("`[package].name` must be present"),
        "expected workspace graph to normalize missing package-name member errors, got:\n{stdout}"
    );
}

#[test]
fn project_graph_explains_unresolved_reference_manifests_and_packages() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-graph-unresolved-reference-detail");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create app source directory for unresolved reference graph test");
    std::fs::create_dir_all(temp.path().join("workspace").join("dep"))
        .expect("create valid dependency directory for unresolved reference graph test");
    std::fs::create_dir_all(temp.path().join("workspace").join("workspace_ref"))
        .expect("create workspace-only reference directory for unresolved reference graph test");
    std::fs::create_dir_all(temp.path().join("workspace").join("broken_ref"))
        .expect("create broken reference directory for unresolved reference graph test");

    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep", "../workspace_ref", "../broken_ref"]
"#,
    );
    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub fn exported() -> Int
"#,
    );
    temp.write(
        "workspace/workspace_ref/qlang.toml",
        r#"
[workspace]
members = ["packages/demo"]
"#,
    );
    temp.write(
        "workspace/broken_ref/qlang.toml",
        r#"
[package
name = "broken_ref"
"#,
    );

    let mut command = ql_command(&workspace_root);
    command.args(["project", "graph"]).arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql project graph` unresolved reference details",
    );
    let (stdout, stderr) = expect_success(
        "project-graph-unresolved-reference-detail",
        "project graph rendering with unresolved reference details",
        &output,
    )
    .expect("project graph with unresolved reference details should succeed");
    expect_empty_stderr(
        "project-graph-unresolved-reference-detail",
        "project graph rendering with unresolved reference details",
        &stderr,
    )
    .expect("project graph unresolved reference detail rendering should stay silent on stderr");
    let normalized_stdout = stdout.replace('\\', "/");
    expect_stdout_contains_all(
        "project-graph-unresolved-reference-detail",
        &normalized_stdout,
        &[
            "reference: ../dep",
            "manifest: ../dep/qlang.toml",
            "status: valid",
            "reference: ../workspace_ref",
            "manifest: ../workspace_ref/qlang.toml",
            "status: unresolved-package",
            "detail: manifest `../workspace_ref/qlang.toml` does not declare `[package].name`",
            "reference: ../broken_ref",
            "manifest: ../broken_ref/qlang.toml",
            "status: unresolved-manifest",
            "detail: invalid manifest `../broken_ref/qlang.toml`:",
        ],
    )
    .expect("project graph should explain unresolved reference manifest and package failures");
}

#[test]
fn project_graph_reports_transitive_reference_failures_for_direct_dependencies() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-graph-transitive-reference-failures");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create app source directory for transitive reference graph test");
    std::fs::create_dir_all(temp.path().join("workspace").join("dep").join("src"))
        .expect("create dependency source directory for transitive reference graph test");
    std::fs::create_dir_all(temp.path().join("workspace").join("broken_ref"))
        .expect("create broken reference directory for transitive reference graph test");

    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"

[references]
packages = ["../broken_ref"]
"#,
    );
    temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub fn exported() -> Int
"#,
    );
    temp.write(
        "workspace/broken_ref/qlang.toml",
        r#"
[package
name = "broken_ref"
"#,
    );

    let mut command = ql_command(&workspace_root);
    command.args(["project", "graph"]).arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql project graph` transitive reference failures",
    );
    let (stdout, stderr) = expect_success(
        "project-graph-transitive-reference-failures",
        "project graph rendering with transitive reference failures",
        &output,
    )
    .expect("project graph with transitive reference failures should succeed");
    expect_empty_stderr(
        "project-graph-transitive-reference-failures",
        "project graph rendering with transitive reference failures",
        &stderr,
    )
    .expect("project graph with transitive reference failures should stay silent on stderr");
    let normalized_stdout = stdout.replace('\\', "/");
    expect_stdout_contains_all(
        "project-graph-transitive-reference-failures",
        &normalized_stdout,
        &[
            "reference: ../dep",
            "manifest: ../dep/qlang.toml",
            "status: valid",
            "transitive_reference_failures: 1",
            "first_transitive_failure_manifest: ../broken_ref/qlang.toml",
            "first_transitive_failure_status: unresolved-manifest",
            "first_transitive_failure_detail: invalid manifest `../broken_ref/qlang.toml`:",
        ],
    )
    .expect("project graph should summarize transitive reference failures on direct dependencies");
}

#[test]
fn project_graph_reports_transitive_stale_reference_reasons_for_direct_dependencies() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-graph-transitive-stale-reference-failures");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create app source directory for transitive stale graph test");
    std::fs::create_dir_all(temp.path().join("workspace").join("dep").join("src"))
        .expect("create dependency source directory for transitive stale graph test");
    std::fs::create_dir_all(temp.path().join("workspace").join("leaf").join("src"))
        .expect("create leaf source directory for transitive stale graph test");

    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"

[references]
packages = ["../leaf"]
"#,
    );
    temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub fn exported() -> Int
"#,
    );
    temp.write(
        "workspace/leaf/qlang.toml",
        r#"
[package]
name = "leaf"
"#,
    );
    temp.write(
        "workspace/leaf/src/lib.ql",
        r#"
pub fn exported() -> Int {
    1
}
"#,
    );
    temp.write(
        "workspace/leaf/leaf.qi",
        r#"
// qlang interface v1
// package: leaf

// source: src/lib.ql
package demo.leaf

pub fn exported() -> Int
"#,
    );

    std::thread::sleep(std::time::Duration::from_millis(1200));

    temp.write(
        "workspace/leaf/src/lib.ql",
        r#"
pub fn exported() -> Int {
    2
}
"#,
    );

    let mut command = ql_command(&workspace_root);
    command.args(["project", "graph"]).arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql project graph` transitive stale reference failures",
    );
    let (stdout, stderr) = expect_success(
        "project-graph-transitive-stale-reference-failures",
        "project graph rendering with transitive stale reference failures",
        &output,
    )
    .expect("project graph with transitive stale reference failures should succeed");
    expect_empty_stderr(
        "project-graph-transitive-stale-reference-failures",
        "project graph rendering with transitive stale reference failures",
        &stderr,
    )
    .expect("project graph with transitive stale reference failures should stay silent on stderr");
    let normalized_stdout = stdout.replace('\\', "/");
    expect_stdout_contains_all(
        "project-graph-transitive-stale-reference-failures",
        &normalized_stdout,
        &[
            "reference: ../dep",
            "manifest: ../dep/qlang.toml",
            "status: valid",
            "transitive_reference_failures: 1",
            "first_transitive_failure_manifest: ../leaf/qlang.toml",
            "first_transitive_failure_path: ../leaf/leaf.qi",
            "first_transitive_failure_status: stale",
            "first_transitive_failure_stale_reasons:",
            "- source: ../leaf/src/lib.ql",
        ],
    )
    .expect("project graph should summarize the first transitive stale reference reason");
}
