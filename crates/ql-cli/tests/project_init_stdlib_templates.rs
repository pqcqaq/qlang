mod support;

use std::fs;

use support::project_init_stdlib::write_repo_stdlib_fixture;
use support::{
    TempDir, expect_empty_stderr, expect_exit_code, expect_stderr_contains, expect_success,
    ql_command, read_normalized_file, run_command_capture, workspace_root,
};

#[test]
fn project_init_with_stdlib_copies_starter_template_from_stdlib_path() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-init-stdlib-template-copy");
    let stdlib_root = write_repo_stdlib_fixture(&temp, &workspace_root);
    temp.write(
        "stdlib/examples/starter/src/lib.ql",
        "pub fn run() -> Int {\n    return 17\n}\n",
    );
    temp.write(
        "stdlib/examples/starter/src/main.ql",
        "fn main() -> Int {\n    return 18\n}\n",
    );
    temp.write(
        "stdlib/examples/starter/tests/smoke.ql",
        "fn main() -> Int {\n    return 19\n}\n",
    );
    let project_root = temp.path().join("demo-package");

    let mut init = ql_command(&workspace_root);
    init.args([
        "project",
        "init",
        &project_root.to_string_lossy(),
        "--stdlib",
        &stdlib_root.to_string_lossy(),
    ]);
    let output = run_command_capture(&mut init, "`ql project init --stdlib` template copy");
    let (_stdout, stderr) = expect_success(
        "project-init-stdlib-template-copy",
        "stdlib package template copy",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-template-copy",
        "stdlib package template copy",
        &stderr,
    )
    .unwrap();

    assert_eq!(
        read_normalized_file(
            &project_root.join("src/lib.ql"),
            "copied stdlib package source"
        ),
        "pub fn run() -> Int {\n    return 17\n}\n"
    );
    assert_eq!(
        read_normalized_file(
            &project_root.join("src/main.ql"),
            "copied stdlib package main source"
        ),
        "fn main() -> Int {\n    return 18\n}\n"
    );
    assert_eq!(
        read_normalized_file(
            &project_root.join("tests/smoke.ql"),
            "copied stdlib package smoke test"
        ),
        "fn main() -> Int {\n    return 19\n}\n"
    );
}

#[test]
fn project_init_workspace_with_missing_stdlib_starter_fails_without_partial_manifest() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-init-stdlib-missing-starter");
    let stdlib_root = write_repo_stdlib_fixture(&temp, &workspace_root);
    fs::remove_dir_all(stdlib_root.join("examples").join("starter"))
        .expect("remove stdlib starter template from fixture");
    let project_root = temp.path().join("demo-workspace");

    let mut init = ql_command(&workspace_root);
    init.args([
        "project",
        "init",
        &project_root.to_string_lossy(),
        "--workspace",
        "--name",
        "app",
        "--stdlib",
        &stdlib_root.to_string_lossy(),
    ]);
    let output = run_command_capture(
        &mut init,
        "`ql project init --workspace --stdlib` missing starter",
    );
    let (_stdout, stderr) = expect_exit_code(
        "project-init-stdlib-missing-starter",
        "missing stdlib starter workspace init",
        &output,
        1,
    )
    .unwrap();
    expect_stderr_contains(
        "project-init-stdlib-missing-starter",
        "missing stdlib starter workspace init",
        &stderr,
        "stdlib starter template",
    )
    .unwrap();
    assert!(
        !project_root.join("qlang.toml").exists(),
        "workspace init should not write a partial root manifest when the stdlib starter is missing"
    );
}
