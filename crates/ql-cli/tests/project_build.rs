mod support;

use support::{
    TempDir, expect_empty_stderr, expect_empty_stdout, expect_exit_code, expect_file_exists,
    expect_stdout_contains_all, expect_success, ql_command, run_command_capture,
    static_library_output_path, workspace_root,
};

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
fn build_workspace_path_builds_each_member_target() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-build-workspace");
    let project_root = temp.path().join("workspace");
    std::fs::create_dir_all(project_root.join("packages/app/src"))
        .expect("create app package source tree");
    std::fs::create_dir_all(project_root.join("packages/tool/src"))
        .expect("create tool package source tree");

    temp.write(
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

    let app_output =
        static_library_output_path(&project_root.join("packages/app/target/ql/debug"), "lib");
    let tool_output =
        static_library_output_path(&project_root.join("packages/tool/target/ql/debug"), "lib");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["build"]).arg(&project_root);
    let output = run_command_capture(&mut command, "`ql build` workspace path");
    let (stdout, stderr) =
        expect_success("project-build-workspace", "workspace path build", &output)
            .expect("workspace path build should succeed");
    expect_empty_stderr("project-build-workspace", "workspace path build", &stderr)
        .expect("workspace path build should not print stderr");
    expect_stdout_contains_all(
        "project-build-workspace",
        &stdout.replace('\\', "/"),
        &[
            &format!("wrote staticlib: {}", app_output.display()).replace('\\', "/"),
            &format!("wrote staticlib: {}", tool_output.display()).replace('\\', "/"),
        ],
    )
    .expect("workspace path build should report each member artifact");

    expect_file_exists(
        "project-build-workspace",
        &app_output,
        "workspace app artifact",
        "workspace path build",
    )
    .expect("workspace path build should emit the app artifact");
    expect_file_exists(
        "project-build-workspace",
        &tool_output,
        "workspace tool artifact",
        "workspace path build",
    )
    .expect("workspace path build should emit the tool artifact");
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
