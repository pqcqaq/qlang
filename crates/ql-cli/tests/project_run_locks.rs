mod support;

use std::process::Stdio;
use std::time::{Duration, Instant};

use ql_driver::{ToolchainOptions, discover_toolchain};
use support::{
    TempDir, assert_no_build_lock_directories, executable_output_path, expect_empty_stderr,
    expect_success, ql_command, sleep_program_source, wait_for_path_exists, workspace_root,
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
