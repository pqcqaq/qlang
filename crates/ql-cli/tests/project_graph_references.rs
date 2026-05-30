mod support;

use support::{
    TempDir, expect_empty_stderr, expect_stdout_contains_all, expect_success, ql_command,
    run_command_capture, workspace_root,
};

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
