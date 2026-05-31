mod support;

use ql_driver::{ToolchainOptions, discover_toolchain};
use support::{
    TempDir, executable_output_path, expect_exit_code, expect_file_exists, expect_silent_output,
    ql_command, run_command_capture, workspace_root,
};

fn toolchain_available(context: &str) -> bool {
    let Ok(_toolchain) = discover_toolchain(&ToolchainOptions::default()) else {
        eprintln!(
            "skipping {context}: no clang-style compiler found via ql-driver toolchain discovery"
        );
        return false;
    };
    true
}

#[test]
fn run_package_path_executes_the_only_runnable_target_with_program_args() {
    if !toolchain_available("`ql run` package test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-package");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create package source tree for run test");
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write("app/src/lib.ql", "pub fn helper() -> Int { return 1 }\n");
    temp.write("app/src/main.ql", "fn main() -> Int { return 9 }\n");
    let output_path = executable_output_path(&project_root.join("target/ql/debug"), "main");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["run"])
        .arg(&project_root)
        .arg("--")
        .args(["alpha", "beta"]);
    let output = run_command_capture(&mut command, "`ql run` package path");
    let (stdout, stderr) = expect_exit_code("project-run-package", "package path run", &output, 9)
        .expect("package-path `ql run` should exit with the runnable target status");
    expect_silent_output("project-run-package", "package path run", &stdout, &stderr)
        .expect("package-path `ql run` should leave stdout/stderr to the program");
    expect_file_exists(
        "project-run-package",
        &output_path,
        "package executable",
        "package path run",
    )
    .expect("package-path `ql run` should leave the built executable in the package target dir");
}

#[test]
fn run_workspace_path_executes_the_only_runnable_target() {
    if !toolchain_available("`ql run` workspace test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-workspace");
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
        "workspace/packages/app/src/main.ql",
        "fn main() -> Int { return 11 }\n",
    );
    temp.write(
        "workspace/packages/tool/src/lib.ql",
        "pub fn helper() -> Int { return 2 }\n",
    );
    let output_path =
        executable_output_path(&project_root.join("packages/app/target/ql/debug"), "main");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["run"]).arg(&project_root);
    let output = run_command_capture(&mut command, "`ql run` workspace path");
    let (stdout, stderr) =
        expect_exit_code("project-run-workspace", "workspace path run", &output, 11)
            .expect("workspace-path `ql run` should exit with the runnable member status");
    expect_silent_output(
        "project-run-workspace",
        "workspace path run",
        &stdout,
        &stderr,
    )
    .expect("workspace-path `ql run` should leave stdout/stderr to the program");
    expect_file_exists(
        "project-run-workspace",
        &output_path,
        "workspace executable",
        "workspace path run",
    )
    .expect("workspace-path `ql run` should leave the built executable in the member target dir");
}
