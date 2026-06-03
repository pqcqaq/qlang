mod support;

use support::{
    TempDir, expect_empty_stderr, expect_empty_stdout, expect_exit_code, expect_stderr_contains,
    expect_success, ql_command, read_normalized_file, run_command_capture, workspace_root,
};

#[test]
fn project_add_dependency_refuses_name_and_path_together() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-add-dependency-conflict");
    let project_root = temp.path().join("workspace");

    let mut init = ql_command(&workspace_root);
    init.args([
        "project",
        "init",
        &project_root.to_string_lossy(),
        "--workspace",
        "--name",
        "app",
    ]);
    let output = run_command_capture(
        &mut init,
        "`ql project init` workspace for conflicting add-dependency selectors",
    );
    let (_stdout, stderr) = expect_success(
        "project-add-dependency-conflict",
        "workspace init for conflicting add-dependency selectors",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-add-dependency-conflict",
        "workspace init for conflicting add-dependency selectors",
        &stderr,
    )
    .unwrap();

    let mut add_dependency = ql_command(&workspace_root);
    add_dependency.args([
        "project",
        "add-dependency",
        &project_root.to_string_lossy(),
        "--package",
        "app",
        "--name",
        "core",
        "--path",
        &project_root.join("vendor/core").to_string_lossy(),
    ]);
    let output = run_command_capture(
        &mut add_dependency,
        "`ql project add-dependency` conflicting selectors",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-add-dependency-conflict",
        "add dependency with conflicting selectors",
        &output,
        1,
    )
    .unwrap();
    expect_empty_stdout(
        "project-add-dependency-conflict",
        "add dependency with conflicting selectors",
        &stdout,
    )
    .unwrap();
    expect_stderr_contains(
        "project-add-dependency-conflict",
        "add dependency with conflicting selectors",
        &stderr,
        "error: `ql project add-dependency` accepts either `--name <package>` or `--path <file-or-dir>`, not both",
    )
    .unwrap();
}

#[test]
fn project_add_dependency_refuses_missing_workspace_package() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-add-dependency-missing");
    let project_root = temp.path().join("workspace");
    let request_path = project_root.join("packages/app/src/main.ql");

    let mut init = ql_command(&workspace_root);
    init.args([
        "project",
        "init",
        &project_root.to_string_lossy(),
        "--workspace",
        "--name",
        "app",
    ]);
    let output = run_command_capture(
        &mut init,
        "`ql project init` workspace for missing add-dependency",
    );
    let (_stdout, stderr) = expect_success(
        "project-add-dependency-missing",
        "workspace init for missing add-dependency",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-add-dependency-missing",
        "workspace init for missing add-dependency",
        &stderr,
    )
    .unwrap();

    let mut add_dependency = ql_command(&workspace_root);
    add_dependency.args([
        "project",
        "add-dependency",
        &request_path.to_string_lossy(),
        "--name",
        "core",
    ]);
    let output = run_command_capture(
        &mut add_dependency,
        "`ql project add-dependency` missing workspace package",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-add-dependency-missing",
        "add dependency with missing workspace package",
        &output,
        1,
    )
    .unwrap();
    expect_empty_stdout(
        "project-add-dependency-missing",
        "add dependency with missing workspace package",
        &stdout,
    )
    .unwrap();
    expect_stderr_contains(
        "project-add-dependency-missing",
        "add dependency with missing workspace package",
        &stderr.replace('\\', "/"),
        &format!(
            "error: `ql project add-dependency` workspace manifest `{}` does not contain package `core`",
            project_root.join("qlang.toml").to_string_lossy().replace('\\', "/")
        ),
    )
    .unwrap();
}

#[test]
fn project_add_dependency_refuses_ambiguous_workspace_package() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-add-dependency-ambiguous");
    let project_root = temp.path().join("workspace");
    let request_path = project_root.join("packages/app/src/main.ql");

    temp.write(
        "workspace/qlang.toml",
        "[workspace]\nmembers = [\"packages/app\", \"packages/a\", \"packages/b\"]\n",
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        "[package]\nname = \"app\"\n",
    );
    temp.write(
        "workspace/packages/app/src/main.ql",
        "fn main() -> Int {\n    return 0\n}\n",
    );
    temp.write(
        "workspace/packages/a/qlang.toml",
        "[package]\nname = \"util\"\n",
    );
    temp.write(
        "workspace/packages/b/qlang.toml",
        "[package]\nname = \"util\"\n",
    );

    let mut add_dependency = ql_command(&workspace_root);
    add_dependency.args([
        "project",
        "add-dependency",
        &request_path.to_string_lossy(),
        "--name",
        "util",
    ]);
    let output = run_command_capture(
        &mut add_dependency,
        "`ql project add-dependency` ambiguous workspace package",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-add-dependency-ambiguous",
        "add dependency with ambiguous workspace package",
        &output,
        1,
    )
    .unwrap();
    expect_empty_stdout(
        "project-add-dependency-ambiguous",
        "add dependency with ambiguous workspace package",
        &stdout,
    )
    .unwrap();
    expect_stderr_contains(
        "project-add-dependency-ambiguous",
        "add dependency with ambiguous workspace package",
        &stderr.replace('\\', "/"),
        &format!(
            "error: `ql project add-dependency` workspace manifest `{}` contains multiple members for package `util`: packages/a, packages/b",
            project_root.join("qlang.toml").to_string_lossy().replace('\\', "/")
        ),
    )
    .unwrap();
}

#[test]
fn project_add_dependency_refuses_unresolved_workspace_member_metadata() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-add-dependency-broken-member");
    let project_root = temp.path().join("workspace");
    let request_path = project_root.join("packages/app/src/main.ql");

    temp.write(
        "workspace/qlang.toml",
        "[workspace]\nmembers = [\"packages/app\", \"packages/core\", \"packages/broken\"]\n",
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        "[package]\nname = \"app\"\n",
    );
    temp.write(
        "workspace/packages/app/src/main.ql",
        "fn main() -> Int {\n    return 0\n}\n",
    );
    temp.write(
        "workspace/packages/core/qlang.toml",
        "[package]\nname = \"core\"\n",
    );
    temp.write(
        "workspace/packages/broken/qlang.toml",
        "[package]\nversion = \"0.1.0\"\n",
    );

    let mut add_dependency = ql_command(&workspace_root);
    add_dependency.args([
        "project",
        "add-dependency",
        &request_path.to_string_lossy(),
        "--name",
        "core",
    ]);
    let output = run_command_capture(
        &mut add_dependency,
        "`ql project add-dependency` broken workspace member metadata",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-add-dependency-broken-member",
        "add dependency with broken workspace member metadata",
        &output,
        1,
    )
    .unwrap();
    expect_empty_stdout(
        "project-add-dependency-broken-member",
        "add dependency with broken workspace member metadata",
        &stdout,
    )
    .unwrap();
    expect_stderr_contains(
        "project-add-dependency-broken-member",
        "add dependency with broken workspace member metadata",
        &stderr.replace('\\', "/"),
        "error: `ql project add-dependency` failed to inspect workspace member `packages/broken`: manifest",
    )
    .unwrap();
    expect_stderr_contains(
        "project-add-dependency-broken-member",
        "add dependency with broken workspace member metadata",
        &stderr,
        "does not declare `[package].name`",
    )
    .unwrap();
    assert_eq!(
        read_normalized_file(
            &project_root.join("packages/app/qlang.toml"),
            "workspace member manifest after rejected add-dependency"
        ),
        "[package]\nname = \"app\"\n"
    );
}
