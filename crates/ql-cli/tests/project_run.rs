mod support;

use std::process::Stdio;
use std::time::{Duration, Instant};

use ql_driver::{ToolchainOptions, discover_toolchain};
use support::{
    TempDir, assert_no_build_lock_directories, executable_output_path, expect_empty_stderr,
    expect_exit_code, expect_file_exists, expect_silent_output, expect_success, ql_command,
    run_command_capture, sleep_program_source, wait_for_path_exists, workspace_root,
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

#[test]
fn run_project_json_holds_executable_lock_while_program_runs() {
    if !toolchain_available("`ql run --json` executable lock during execution test") {
        return;
    }

    let temp = TempDir::new("ql-project-run-json-executable-lock-during-execution");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create package source tree for executable lock run test");
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write("app/src/main.ql", &sleep_program_source(900));
    let executable_output = executable_output_path(&project_root.join("target/ql/debug"), "main");
    let workspace_root = workspace_root();

    let mut first = ql_command(&workspace_root);
    first.current_dir(temp.path());
    first.args(["run", "--json"]).arg(&project_root);
    first.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut first = first
        .spawn()
        .expect("spawn first long-running `ql run --json`");
    wait_for_path_exists(
        "project-run-json-executable-lock-during-execution",
        "first long-running run executable",
        &executable_output,
        Duration::from_secs(20),
    )
    .expect("first long-running run should create the executable before execution");
    std::thread::sleep(Duration::from_millis(50));
    assert!(
        first
            .try_wait()
            .expect("poll first long-running `ql run --json`")
            .is_none(),
        "first long-running run should still hold the executable while the second run starts"
    );

    let mut second = ql_command(&workspace_root);
    second.current_dir(temp.path());
    second.args(["run", "--json"]).arg(&project_root);
    second.stdout(Stdio::piped()).stderr(Stdio::piped());
    let second_started = Instant::now();
    let second = second
        .spawn()
        .expect("spawn second `ql run --json` against same executable");

    let second_output = second
        .wait_with_output()
        .expect("wait for second executable-lock `ql run --json`");
    let second_elapsed = second_started.elapsed();
    let first_output = first
        .wait_with_output()
        .expect("wait for first executable-lock `ql run --json`");

    for (label, output) in [("first", first_output), ("second", second_output)] {
        let (stdout, stderr) = expect_success(
            "project-run-json-executable-lock-during-execution",
            &format!("{label} long-running executable-lock run"),
            &output,
        )
        .unwrap_or_else(|message| panic!("{message}"));
        expect_empty_stderr(
            "project-run-json-executable-lock-during-execution",
            &format!("{label} long-running executable-lock run"),
            &stderr,
        )
        .unwrap_or_else(|message| panic!("{message}"));
        let json: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|error| {
            panic!("[project-run-json-executable-lock-during-execution] parse json stdout: {error}\n{stdout}")
        });
        assert_eq!(json["schema"], "ql.run.v1");
        assert_eq!(json["status"], "completed");
        assert_eq!(json["execution"]["exit_code"], 0);
    }

    assert!(
        second_elapsed >= Duration::from_millis(1000),
        "second run should wait for the first execution lock before rebuilding the same executable; elapsed {second_elapsed:?}"
    );
    assert_no_build_lock_directories(
        "project-run-json-executable-lock-during-execution",
        &project_root,
    );
}

#[test]
fn run_preserves_large_exit_code() {
    if !toolchain_available("`ql run` large-exit-code test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-large-exit");
    let source_path = temp.write("large_exit.ql", "fn main() -> Int { return 690 }\n");
    let output_path = executable_output_path(&temp.path().join("target/ql/debug"), "large_exit");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["run"]).arg(&source_path);
    let output = run_command_capture(&mut command, "`ql run` large exit code");
    let (stdout, stderr) = expect_exit_code(
        "project-run-large-exit",
        "large-exit-code run",
        &output,
        690,
    )
    .expect("`ql run` should preserve the child exit code");
    expect_silent_output(
        "project-run-large-exit",
        "large-exit-code run",
        &stdout,
        &stderr,
    )
    .expect("large-exit-code `ql run` should leave stdout/stderr to the program");
    expect_file_exists(
        "project-run-large-exit",
        &output_path,
        "large-exit executable",
        "large-exit-code run",
    )
    .expect("large-exit-code `ql run` should still leave the built executable in place");
    assert_no_build_lock_directories("project-run-large-exit", temp.path());
}

#[test]
fn run_project_path_selects_requested_binary_target() {
    if !toolchain_available("`ql run --bin` package test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-select-bin");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src/bin"))
        .expect("create package source tree for target selector run test");
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write("app/src/main.ql", "fn main() -> Int { return 1 }\n");
    temp.write("app/src/bin/admin.ql", "fn main() -> Int { return 2 }\n");
    let output_path = executable_output_path(&project_root.join("target/ql/debug/bin"), "admin");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["run"])
        .arg(&project_root)
        .args(["--bin", "admin"]);
    let output = run_command_capture(&mut command, "`ql run --bin` package path");
    let (stdout, stderr) = expect_exit_code(
        "project-run-select-bin",
        "selected binary target run",
        &output,
        2,
    )
    .expect("package-path `ql run --bin` should exit with the selected binary status");
    expect_silent_output(
        "project-run-select-bin",
        "selected binary target run",
        &stdout,
        &stderr,
    )
    .expect("package-path `ql run --bin` should leave stdout/stderr to the program");
    expect_file_exists(
        "project-run-select-bin",
        &output_path,
        "selected binary executable",
        "selected binary target run",
    )
    .expect(
        "package-path `ql run --bin` should build the selected executable in the bin target dir",
    );
}
