mod support;

use support::{
    TempDir, expect_empty_stdout, expect_exit_code, expect_file_exists, expect_snapshot_matches,
    expect_stderr_contains, expect_success, ql_command, read_normalized_file, run_command_capture,
    workspace_root,
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
fn project_emit_interface_rejects_workspace_only_manifest() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-interface-workspace-only");
    let project_root = temp.path().join("workspace-only");
    std::fs::create_dir_all(&project_root).expect("create workspace-only test directory");
    temp.write(
        "workspace-only/qlang.toml",
        r#"
[workspace]
members = ["packages/app"]
"#,
    );

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "emit-interface"])
        .arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql project emit-interface` workspace-only manifest",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-interface-workspace-only",
        "workspace-only interface emission",
        &output,
        1,
    )
    .expect("workspace-only interface emission should fail");
    expect_empty_stdout(
        "project-interface-workspace-only",
        "workspace-only interface emission",
        &stdout,
    )
    .expect("workspace-only interface emission should not print stdout");
    expect_stderr_contains(
        "project-interface-workspace-only",
        "workspace-only interface emission",
        &stderr,
        "does not declare `[package].name`",
    )
    .expect("workspace-only manifest failure should mention the package contract");
}
