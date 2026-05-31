use std::path::PathBuf;

use super::{
    TempDir, executable_output_path, expect_exit_code, expect_file_exists, expect_silent_output,
    ql_command, run_command_capture, static_library_output_path, workspace_root,
};

pub struct DependencyRunProject {
    pub temp: TempDir,
    pub project_root: PathBuf,
    pub interface_output: PathBuf,
    pub dependency_output: PathBuf,
    pub executable_output: PathBuf,
}

pub fn write_dependency_run_project(
    prefix: &str,
    dependency_source: &str,
    app_source: &str,
) -> DependencyRunProject {
    let temp = TempDir::new(prefix);
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
    temp.write("dep/src/lib.ql", dependency_source);
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"

[dependencies]
dep = "../dep"
"#,
    );
    temp.write("app/src/main.ql", app_source);

    let interface_output = dep_root.join("dep.qi");
    let dependency_output = static_library_output_path(&dep_root.join("target/ql/debug"), "lib");
    let executable_output = executable_output_path(&project_root.join("target/ql/debug"), "main");
    assert!(
        !interface_output.exists(),
        "dependency interface should start missing for {prefix}"
    );

    DependencyRunProject {
        temp,
        project_root,
        interface_output,
        dependency_output,
        executable_output,
    }
}

pub fn expect_dependency_run_project_exits(
    case_name: &str,
    action: &str,
    command_description: &str,
    fixture: &DependencyRunProject,
    expected_exit_code: i32,
) {
    let workspace_root = workspace_root();
    let mut command = ql_command(&workspace_root);
    command.current_dir(fixture.temp.path());
    command.args(["run"]).arg(&fixture.project_root);
    let output = run_command_capture(&mut command, command_description);
    let (stdout, stderr) = expect_exit_code(case_name, action, &output, expected_exit_code)
        .expect("dependency run project should exit with the expected program status");
    expect_silent_output(case_name, action, &stdout, &stderr)
        .expect("dependency run project should leave stdout/stderr to the program");
    expect_file_exists(
        case_name,
        &fixture.interface_output,
        "synced dependency interface",
        action,
    )
    .expect("dependency run project should emit the dependency interface");
    expect_file_exists(
        case_name,
        &fixture.dependency_output,
        "dependency package artifact",
        action,
    )
    .expect("dependency run project should build the dependency package artifact");
    expect_file_exists(
        case_name,
        &fixture.executable_output,
        "package executable",
        action,
    )
    .expect("dependency run project should emit the executable artifact");
}
