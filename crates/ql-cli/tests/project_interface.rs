mod support;

use support::{
    TempDir, expect_empty_stdout, expect_exit_code, expect_file_exists, expect_snapshot_matches,
    expect_stderr_contains, expect_stdout_contains_all, expect_success, ql_command,
    read_normalized_file, run_command_capture, workspace_root,
};

#[test]
fn project_emit_interface_writes_public_qi_surface() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-interface");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(project_root.join("src").join("nested"))
        .expect("create project source directory for interface emit test");
    let interface_path = project_root.join("app.qi");
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace/app/src/lib.ql",
        r#"
package demo.api

pub const DEFAULT_PORT: Int = 8080
const INTERNAL_PORT: Int = 3000

pub struct Buffer[T] {
    value: T,
    count: Int = 0,
}

pub trait Writer {
    fn flush(var self) -> Int
}

impl Buffer[Int] {
    pub fn len(self) -> Int {
        return 1
    }

    fn hidden(self) -> Int {
        return 0
    }
}

extend Buffer[Int] {
    pub fn twice(self) -> Int {
        return 2
    }

    fn private_twice(self) -> Int {
        return 1
    }
}

pub fn sum(left: Int, right: Int) -> Int {
    return left + right
}

pub extern "c" {
    fn puts(ptr: *const U8) -> I32
}
"#,
    );
    temp.write(
        "workspace/app/src/nested/types.ql",
        r#"
package demo.api

pub static BUILD_ID: Int = 1

pub enum Shape {
    Unit,
    Pair(Int, Int),
}

pub type Pair = (Int, Int)
"#,
    );

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "emit-interface"])
        .arg(&project_root);
    let output = run_command_capture(&mut command, "`ql project emit-interface`");
    let (stdout, stderr) = expect_success(
        "project-interface-success",
        "project interface emission",
        &output,
    )
    .expect("project interface emission should succeed");
    expect_snapshot_matches(
        "project-interface-success",
        "project interface stdout",
        &format!("wrote interface: {}\n", interface_path.display()),
        &stdout,
    )
    .expect("interface emission should report the written artifact path");
    expect_snapshot_matches(
        "project-interface-success",
        "project interface stderr",
        "",
        &stderr,
    )
    .expect("successful interface emission should stay silent on stderr");
    expect_file_exists(
        "project-interface-success",
        &interface_path,
        "generated interface",
        "project interface emission",
    )
    .expect("interface emission should create the default package qi artifact");

    let expected = "\
// qlang interface v1
// package: app

// source: src/lib.ql
package demo.api

pub const DEFAULT_PORT: Int

pub struct Buffer[T] {
    value: T,
    count: Int,
}

pub trait Writer {
    fn flush(var self) -> Int
}

impl Buffer[Int] {
    pub fn len(self) -> Int
}

extend Buffer[Int] {
    pub fn twice(self) -> Int
}

pub fn sum(left: Int, right: Int) -> Int

pub extern \"c\" {
    fn puts(ptr: *const U8) -> I32
}

// source: src/nested/types.ql
package demo.api

pub static BUILD_ID: Int

pub enum Shape {
    Unit,
    Pair(Int, Int),
}

pub type Pair = (Int, Int)
";
    let actual = read_normalized_file(&interface_path, "generated qi artifact");
    expect_snapshot_matches(
        "project-interface-success",
        "generated qi artifact",
        expected,
        &actual,
    )
    .expect("generated qi artifact should match the public interface snapshot");
}

#[test]
fn project_emit_interface_changed_only_skips_up_to_date_package_interface() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-interface-changed-only-package");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create project source directory for changed-only package test");
    let interface_path = project_root.join("app.qi");
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace/app/src/lib.ql",
        r#"
package demo.app

pub fn exported() -> Int {
    return 1
}
"#,
    );
    let expected = "\
// qlang interface v1
// package: app

// source: src/lib.ql
package demo.app

pub fn exported() -> Int
";
    temp.write("workspace/app/app.qi", expected);

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "emit-interface", "--changed-only"])
        .arg(&project_root);
    let output = run_command_capture(&mut command, "`ql project emit-interface --changed-only`");
    let (stdout, stderr) = expect_success(
        "project-interface-changed-only-package",
        "changed-only package interface emission",
        &output,
    )
    .expect("changed-only package interface emission should succeed");
    expect_snapshot_matches(
        "project-interface-changed-only-package",
        "changed-only package interface stdout",
        &format!("up-to-date interface: {}\n", interface_path.display()),
        &stdout,
    )
    .expect("changed-only package interface emission should skip up-to-date artifact");
    expect_snapshot_matches(
        "project-interface-changed-only-package",
        "changed-only package interface stderr",
        "",
        &stderr,
    )
    .expect("changed-only package interface emission should stay silent on stderr");
    let actual = read_normalized_file(&interface_path, "changed-only generated qi artifact");
    expect_snapshot_matches(
        "project-interface-changed-only-package",
        "changed-only package qi artifact",
        expected,
        &actual,
    )
    .expect("changed-only package interface emission should leave up-to-date artifact unchanged");
}

#[test]
fn project_emit_interface_writes_member_qi_for_workspace_only_manifest() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-interface-workspace-only");
    let project_root = temp.path().join("workspace-only");
    let app_root = project_root.join("packages").join("app");
    let tool_root = project_root.join("packages").join("tool");
    std::fs::create_dir_all(app_root.join("src")).expect("create app package source directory");
    std::fs::create_dir_all(tool_root.join("src")).expect("create tool package source directory");
    temp.write(
        "workspace-only/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/tool"]
"#,
    );
    temp.write(
        "workspace-only/packages/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace-only/packages/app/src/lib.ql",
        r#"
package demo.app

pub fn exported() -> Int {
    return 1
}
"#,
    );
    temp.write(
        "workspace-only/packages/tool/qlang.toml",
        r#"
[package]
name = "tool"
"#,
    );
    temp.write(
        "workspace-only/packages/tool/src/lib.ql",
        r#"
package demo.tool

pub struct Config {
    value: Int,
}
"#,
    );
    let app_interface = app_root.join("app.qi");
    let tool_interface = tool_root.join("tool.qi");

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "emit-interface"])
        .arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql project emit-interface` workspace-only manifest",
    );
    let (stdout, stderr) = expect_success(
        "project-interface-workspace-only",
        "workspace-only interface emission",
        &output,
    )
    .expect("workspace-only interface emission should succeed");
    let normalized_stdout = stdout.replace('\\', "/");
    let normalized_app_interface = app_interface.display().to_string().replace('\\', "/");
    let normalized_tool_interface = tool_interface.display().to_string().replace('\\', "/");
    expect_stdout_contains_all(
        "project-interface-workspace-only",
        &normalized_stdout,
        &[
            &format!("wrote interface: {normalized_app_interface}"),
            &format!("wrote interface: {normalized_tool_interface}"),
        ],
    )
    .expect("workspace-only interface emission should report each written artifact");
    expect_snapshot_matches(
        "project-interface-workspace-only",
        "workspace-only interface emission stderr",
        &stderr,
        "",
    )
    .expect("workspace-only interface emission should stay silent on stderr");
    expect_file_exists(
        "project-interface-workspace-only",
        &app_interface,
        "workspace app qi",
        "workspace-only interface emission",
    )
    .expect("workspace-only interface emission should create app qi");
    expect_file_exists(
        "project-interface-workspace-only",
        &tool_interface,
        "workspace tool qi",
        "workspace-only interface emission",
    )
    .expect("workspace-only interface emission should create tool qi");
}

#[test]
fn project_emit_interface_changed_only_rewrites_only_stale_workspace_members() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-interface-changed-only-workspace");
    let project_root = temp.path().join("workspace-only");
    let app_root = project_root.join("packages").join("app");
    let tool_root = project_root.join("packages").join("tool");
    std::fs::create_dir_all(app_root.join("src"))
        .expect("create app package source directory for changed-only workspace test");
    std::fs::create_dir_all(tool_root.join("src"))
        .expect("create tool package source directory for changed-only workspace test");
    temp.write(
        "workspace-only/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/tool"]
"#,
    );
    temp.write(
        "workspace-only/packages/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace-only/packages/app/src/lib.ql",
        r#"
package demo.app

pub fn exported() -> Int {
    return 1
}
"#,
    );
    temp.write(
        "workspace-only/packages/app/app.qi",
        "\
// qlang interface v1
// package: app

// source: src/lib.ql
package demo.app

pub fn exported() -> Int
",
    );
    temp.write(
        "workspace-only/packages/tool/qlang.toml",
        r#"
[package]
name = "tool"
"#,
    );
    temp.write(
        "workspace-only/packages/tool/src/lib.ql",
        r#"
package demo.tool

pub fn exported() -> Int {
    return 1
}
"#,
    );
    temp.write(
        "workspace-only/packages/tool/tool.qi",
        "\
// qlang interface v1
// package: tool

// source: src/lib.ql
package demo.tool

pub fn exported() -> Int
",
    );
    std::thread::sleep(std::time::Duration::from_millis(1200));
    temp.write(
        "workspace-only/packages/tool/src/lib.ql",
        r#"
package demo.tool

pub fn exported() -> Int {
    return 1
}

pub fn newer() -> Int {
    return 2
}
"#,
    );
    let app_interface = app_root.join("app.qi");
    let tool_interface = tool_root.join("tool.qi");

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "emit-interface", "--changed-only"])
        .arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql project emit-interface --changed-only` workspace-only manifest",
    );
    let (stdout, stderr) = expect_success(
        "project-interface-changed-only-workspace",
        "changed-only workspace interface emission",
        &output,
    )
    .expect("changed-only workspace interface emission should succeed");
    let normalized_stdout = stdout.replace('\\', "/");
    let normalized_app_interface = app_interface.display().to_string().replace('\\', "/");
    let normalized_tool_interface = tool_interface.display().to_string().replace('\\', "/");
    expect_stdout_contains_all(
        "project-interface-changed-only-workspace",
        &normalized_stdout,
        &[
            &format!("up-to-date interface: {normalized_app_interface}"),
            &format!("wrote interface: {normalized_tool_interface}"),
        ],
    )
    .expect("changed-only workspace interface emission should skip valid member and rewrite stale member");
    expect_snapshot_matches(
        "project-interface-changed-only-workspace",
        "changed-only workspace interface emission stderr",
        "",
        &stderr,
    )
    .expect("changed-only workspace interface emission should stay silent on stderr");
    let tool_actual = read_normalized_file(&tool_interface, "changed-only workspace tool qi");
    assert!(
        tool_actual.contains("pub fn newer() -> Int"),
        "expected stale workspace member interface to be regenerated, got:\n{tool_actual}"
    );
}

#[test]
fn project_emit_interface_rejects_output_path_for_workspace_only_manifest() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-interface-workspace-output");
    let project_root = temp.path().join("workspace-only");
    std::fs::create_dir_all(project_root.join("packages").join("app").join("src"))
        .expect("create workspace-only package directory");
    temp.write(
        "workspace-only/qlang.toml",
        r#"
[workspace]
members = ["packages/app"]
"#,
    );
    temp.write(
        "workspace-only/packages/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace-only/packages/app/src/lib.ql",
        r#"
package demo.app

pub fn exported() -> Int {
    return 1
}
"#,
    );

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "emit-interface"])
        .arg(&project_root)
        .args(["--output", "workspace.qi"]);
    let output = run_command_capture(
        &mut command,
        "`ql project emit-interface --output` workspace-only manifest",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-interface-workspace-output",
        "workspace-only interface emission with output",
        &output,
        1,
    )
    .expect("workspace-only interface emission with output should fail");
    expect_empty_stdout(
        "project-interface-workspace-output",
        "workspace-only interface emission with output",
        &stdout,
    )
    .expect("workspace-only interface emission with output should not print stdout");
    expect_stderr_contains(
        "project-interface-workspace-output",
        "workspace-only interface emission with output",
        &stderr,
        "--output` only supports package manifests",
    )
    .expect("workspace-only output rejection should explain the package-only constraint");
}

#[test]
fn build_with_emit_interface_writes_default_package_qi() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-build-emit-interface");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create project source directory for build interface test");
    let source_path = temp.write(
        "workspace/app/src/lib.ql",
        r#"
package demo.app

pub struct Buffer {
    value: Int,
}

pub fn exported(value: Int) -> Int {
    return value
}

fn main() -> Int {
    return exported(1)
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    let output_path = project_root.join("build").join("app.ll");
    let interface_path = project_root.join("app.qi");

    let mut command = ql_command(&workspace_root);
    command
        .arg("build")
        .arg(&source_path)
        .args(["--emit", "llvm-ir", "--output"])
        .arg(&output_path)
        .arg("--emit-interface");
    let output = run_command_capture(&mut command, "`ql build --emit-interface`");
    let (stdout, stderr) = expect_success(
        "build-emit-interface-success",
        "build with interface emission",
        &output,
    )
    .expect("build with interface emission should succeed");
    expect_stdout_contains_all(
        "build-emit-interface-success",
        &stdout,
        &[
            &format!("wrote llvm-ir: {}", output_path.display()),
            "wrote interface:",
            "app.qi",
        ],
    )
    .expect("build with interface emission should report both output artifacts");
    expect_snapshot_matches(
        "build-emit-interface-success",
        "build with interface emission stderr",
        "",
        &stderr,
    )
    .expect("successful build with interface emission should stay silent on stderr");
    expect_file_exists(
        "build-emit-interface-success",
        &output_path,
        "generated llvm ir",
        "build with interface emission",
    )
    .expect("build with interface emission should create the requested build artifact");
    expect_file_exists(
        "build-emit-interface-success",
        &interface_path,
        "generated interface",
        "build with interface emission",
    )
    .expect("build with interface emission should create the default package qi artifact");

    let expected = "\
// qlang interface v1
// package: app

// source: src/lib.ql
package demo.app

pub struct Buffer {
    value: Int,
}

pub fn exported(value: Int) -> Int
";
    let actual = read_normalized_file(&interface_path, "generated qi artifact");
    expect_snapshot_matches(
        "build-emit-interface-success",
        "generated qi artifact",
        expected,
        &actual,
    )
    .expect("generated qi artifact should match the build-side public interface snapshot");
}
