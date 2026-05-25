mod support;

use serde_json::Value as JsonValue;
use support::{
    TempDir, expect_empty_stderr, expect_empty_stdout, expect_exit_code, expect_file_exists,
    expect_snapshot_matches, expect_stderr_contains, expect_stdout_contains_all, expect_success,
    ql_command, read_normalized_file, run_command_capture, workspace_root,
};

fn write_maintenance_package(prefix: &str) -> TempDir {
    let temp = TempDir::new(prefix);
    temp.write(
        "qlang.toml",
        r#"
[package]
name = "app"

[[bin]]
path = "src/main.ql"
"#,
    );
    temp.write("src/main.ql", "fn main() -> Int { return 0 }\n");
    temp
}

fn parse_json_stdout(case_name: &str, stdout: &str) -> JsonValue {
    serde_json::from_str(stdout)
        .unwrap_or_else(|error| panic!("[{case_name}] parse json stdout: {error}\n{stdout}"))
}

#[test]
fn project_maintenance_commands_use_current_directory_by_default() {
    let workspace_root = workspace_root();
    let temp = write_maintenance_package("ql-project-maintenance-default-cwd");

    let mut lock = ql_command(&workspace_root);
    lock.current_dir(temp.path());
    lock.args(["project", "lock", "--json"]);
    let output = run_command_capture(&mut lock, "`ql project lock --json` default cwd");
    let (stdout, stderr) = expect_success(
        "project-maintenance-default-cwd",
        "`ql project lock --json` default cwd",
        &output,
    )
    .expect("project lock should use the current directory by default");
    expect_empty_stderr(
        "project-maintenance-default-cwd",
        "`ql project lock --json` default cwd",
        &stderr,
    )
    .expect("project lock default cwd should keep stderr empty");
    let json = parse_json_stdout("project-maintenance-default-cwd", &stdout);
    assert_eq!(json["schema"], "ql.project.lock.result.v1");
    assert_eq!(json["status"], "wrote");
    assert_eq!(json["check_only"], false);
    expect_file_exists(
        "project-maintenance-default-cwd",
        &temp.path().join("qlang.lock"),
        "lockfile",
        "`ql project lock --json` default cwd",
    )
    .expect("project lock default cwd should write the lockfile");

    let mut target_add = ql_command(&workspace_root);
    target_add.current_dir(temp.path());
    target_add.args(["project", "target", "add", "--bin", "worker"]);
    let output = run_command_capture(&mut target_add, "`ql project target add --bin` default cwd");
    let (stdout, stderr) = expect_success(
        "project-maintenance-default-cwd",
        "`ql project target add --bin` default cwd",
        &output,
    )
    .expect("project target add should use the current directory by default");
    expect_empty_stderr(
        "project-maintenance-default-cwd",
        "`ql project target add --bin` default cwd",
        &stderr,
    )
    .expect("project target add default cwd should keep stderr empty");
    expect_stdout_contains_all(
        "project-maintenance-default-cwd",
        &stdout.replace('\\', "/"),
        &[
            &format!(
                "updated: {}",
                temp.path()
                    .join("qlang.toml")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            &format!(
                "created: {}",
                temp.path()
                    .join("src/bin/worker.ql")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
        ],
    )
    .expect("project target add default cwd should report updated and created files");
    expect_file_exists(
        "project-maintenance-default-cwd",
        &temp.path().join("src/bin/worker.ql"),
        "binary target source",
        "`ql project target add --bin` default cwd",
    )
    .expect("project target add default cwd should create the binary target source");
    let manifest = read_normalized_file(&temp.path().join("qlang.toml"), "updated manifest");
    assert!(
        manifest.contains("[[bin]]\npath = \"src/main.ql\"\n")
            && manifest.contains("[[bin]]\npath = \"src/bin/worker.ql\"\n"),
        "project target add default cwd should preserve and append binary targets, got:\n{manifest}"
    );

    let mut emit_interface = ql_command(&workspace_root);
    emit_interface.current_dir(temp.path());
    emit_interface.args(["project", "emit-interface"]);
    let output = run_command_capture(
        &mut emit_interface,
        "`ql project emit-interface` default cwd",
    );
    let (stdout, stderr) = expect_success(
        "project-maintenance-default-cwd",
        "`ql project emit-interface` default cwd",
        &output,
    )
    .expect("project emit-interface should use the current directory by default");
    expect_empty_stderr(
        "project-maintenance-default-cwd",
        "`ql project emit-interface` default cwd",
        &stderr,
    )
    .expect("project emit-interface default cwd should keep stderr empty");
    let interface_path = temp.path().join("app.qi");
    expect_snapshot_matches(
        "project-maintenance-default-cwd",
        "project emit-interface default cwd stdout",
        &format!(
            "wrote interface: {}\n",
            interface_path.display().to_string().replace('\\', "/")
        ),
        &stdout.replace('\\', "/"),
    )
    .expect("project emit-interface default cwd should report the generated interface");
    expect_file_exists(
        "project-maintenance-default-cwd",
        &interface_path,
        "interface artifact",
        "`ql project emit-interface` default cwd",
    )
    .expect("project emit-interface default cwd should create the interface artifact");
    let interface = read_normalized_file(&interface_path, "generated interface artifact");
    assert!(
        interface.contains("// qlang interface v1\n// package: app\n"),
        "project emit-interface default cwd should generate the package interface, got:\n{interface}"
    );
}

#[test]
fn project_lock_cli_rejects_invalid_arguments() {
    let workspace_root = workspace_root();
    let temp = write_maintenance_package("ql-project-lock-cli-invalid");
    let path = temp.path().display().to_string();

    let cases = [
        (
            vec![
                "project".to_owned(),
                "lock".to_owned(),
                path.clone(),
                "--unknown".to_owned(),
            ],
            "error: unknown `ql project lock` option `--unknown`",
        ),
        (
            vec![
                "project".to_owned(),
                "lock".to_owned(),
                path,
                "extra".to_owned(),
            ],
            "error: unknown `ql project lock` argument `extra`",
        ),
    ];

    for (args, expected_error) in cases {
        let mut command = ql_command(&workspace_root);
        command.args(args);
        let output = run_command_capture(&mut command, "`ql project lock` invalid args");
        let (stdout, stderr) = expect_exit_code(
            "project-lock-cli-invalid",
            "`ql project lock` invalid args",
            &output,
            1,
        )
        .expect("project lock should reject invalid arguments");
        expect_empty_stdout(
            "project-lock-cli-invalid",
            "`ql project lock` invalid args",
            &stdout,
        )
        .expect("project lock invalid args should keep stdout empty");
        expect_stderr_contains(
            "project-lock-cli-invalid",
            "`ql project lock` invalid args",
            &stderr,
            expected_error,
        )
        .expect("project lock invalid args should report the parser error");
    }
}

#[test]
fn project_target_cli_rejects_invalid_arguments() {
    let workspace_root = workspace_root();
    let temp = write_maintenance_package("ql-project-target-cli-invalid");
    let path = temp.path().display().to_string();

    let cases = [
        (
            vec!["project".to_owned(), "target".to_owned()],
            "error: `ql project target` expects a subcommand",
        ),
        (
            vec![
                "project".to_owned(),
                "target".to_owned(),
                "unknown".to_owned(),
            ],
            "error: unknown `ql project target` subcommand `unknown`",
        ),
        (
            vec![
                "project".to_owned(),
                "target".to_owned(),
                "add".to_owned(),
                path.clone(),
            ],
            "error: `ql project target add` expects `--bin <name>`",
        ),
        (
            vec![
                "project".to_owned(),
                "target".to_owned(),
                "add".to_owned(),
                path.clone(),
                "--bin".to_owned(),
            ],
            "error: `ql project target add --bin` expects a target name",
        ),
        (
            vec![
                "project".to_owned(),
                "target".to_owned(),
                "add".to_owned(),
                path.clone(),
                "--unknown".to_owned(),
            ],
            "error: unknown `ql project target add` option `--unknown`",
        ),
        (
            vec![
                "project".to_owned(),
                "target".to_owned(),
                "add".to_owned(),
                path.clone(),
                "extra".to_owned(),
                "--bin".to_owned(),
                "worker".to_owned(),
            ],
            "error: unknown `ql project target add` argument `extra`",
        ),
        (
            vec![
                "project".to_owned(),
                "target".to_owned(),
                "add".to_owned(),
                path.clone(),
                "--bin".to_owned(),
                "worker".to_owned(),
                "--bin".to_owned(),
                "admin".to_owned(),
            ],
            "error: `ql project target add` received `--bin` more than once",
        ),
        (
            vec![
                "project".to_owned(),
                "target".to_owned(),
                "add".to_owned(),
                path,
                "--package".to_owned(),
                "app".to_owned(),
                "--package".to_owned(),
                "core".to_owned(),
                "--bin".to_owned(),
                "worker".to_owned(),
            ],
            "error: `ql project target add` received `--package` more than once",
        ),
    ];

    for (args, expected_error) in cases {
        let mut command = ql_command(&workspace_root);
        command.args(args);
        let output = run_command_capture(&mut command, "`ql project target` invalid args");
        let (stdout, stderr) = expect_exit_code(
            "project-target-cli-invalid",
            "`ql project target` invalid args",
            &output,
            1,
        )
        .expect("project target should reject invalid arguments");
        expect_empty_stdout(
            "project-target-cli-invalid",
            "`ql project target` invalid args",
            &stdout,
        )
        .expect("project target invalid args should keep stdout empty");
        expect_stderr_contains(
            "project-target-cli-invalid",
            "`ql project target` invalid args",
            &stderr,
            expected_error,
        )
        .expect("project target invalid args should report the parser error");
    }
}

#[test]
fn project_emit_interface_cli_rejects_invalid_arguments() {
    let workspace_root = workspace_root();
    let temp = write_maintenance_package("ql-project-emit-interface-cli-invalid");
    let path = temp.path().display().to_string();

    let cases = [
        (
            vec![
                "project".to_owned(),
                "emit-interface".to_owned(),
                path.clone(),
                "--package".to_owned(),
            ],
            "error: `ql project emit-interface --package` expects a package name",
        ),
        (
            vec![
                "project".to_owned(),
                "emit-interface".to_owned(),
                path.clone(),
                "--package".to_owned(),
                "app".to_owned(),
                "--package".to_owned(),
                "core".to_owned(),
            ],
            "error: `ql project emit-interface` received `--package` more than once",
        ),
        (
            vec![
                "project".to_owned(),
                "emit-interface".to_owned(),
                path.clone(),
                "--output".to_owned(),
            ],
            "error: `ql project emit-interface --output` expects a file path",
        ),
        (
            vec![
                "project".to_owned(),
                "emit-interface".to_owned(),
                path.clone(),
                "-o".to_owned(),
            ],
            "error: `ql project emit-interface --output` expects a file path",
        ),
        (
            vec![
                "project".to_owned(),
                "emit-interface".to_owned(),
                path.clone(),
                "--unknown".to_owned(),
            ],
            "error: unknown `ql project emit-interface` option `--unknown`",
        ),
        (
            vec![
                "project".to_owned(),
                "emit-interface".to_owned(),
                path.clone(),
                "extra".to_owned(),
            ],
            "error: unknown `ql project emit-interface` argument `extra`",
        ),
        (
            vec![
                "project".to_owned(),
                "emit-interface".to_owned(),
                path,
                "--check".to_owned(),
                "--output".to_owned(),
                "app.qi".to_owned(),
            ],
            "error: `ql project emit-interface --check` does not support `--output`",
        ),
    ];

    for (args, expected_error) in cases {
        let mut command = ql_command(&workspace_root);
        command.args(args);
        let output = run_command_capture(&mut command, "`ql project emit-interface` invalid args");
        let (stdout, stderr) = expect_exit_code(
            "project-emit-interface-cli-invalid",
            "`ql project emit-interface` invalid args",
            &output,
            1,
        )
        .expect("project emit-interface should reject invalid arguments");
        expect_empty_stdout(
            "project-emit-interface-cli-invalid",
            "`ql project emit-interface` invalid args",
            &stdout,
        )
        .expect("project emit-interface invalid args should keep stdout empty");
        expect_stderr_contains(
            "project-emit-interface-cli-invalid",
            "`ql project emit-interface` invalid args",
            &stderr,
            expected_error,
        )
        .expect("project emit-interface invalid args should report the parser error");
    }
}
