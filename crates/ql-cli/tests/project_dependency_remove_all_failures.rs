mod support;

use support::{
    TempDir, expect_empty_stdout, expect_exit_code, expect_stderr_contains, ql_command,
    run_command_capture, workspace_root,
};

#[test]
fn project_remove_dependency_all_refuses_package_selector() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-remove-dependency-all-package");
    let project_root = temp.path().join("workspace");

    temp.write(
        "workspace/qlang.toml",
        "[workspace]\nmembers = [\"packages/app\", \"packages/core\"]\n",
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        "[package]\nname = \"app\"\n\n[dependencies]\ncore = \"../core\"\n",
    );
    temp.write(
        "workspace/packages/core/qlang.toml",
        "[package]\nname = \"core\"\n",
    );

    let mut remove_dependency = ql_command(&workspace_root);
    remove_dependency.args([
        "project",
        "remove-dependency",
        &project_root.to_string_lossy(),
        "--package",
        "app",
        "--name",
        "core",
        "--all",
    ]);
    let output = run_command_capture(
        &mut remove_dependency,
        "`ql project remove-dependency --all --package` workspace root",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-remove-dependency-all-package",
        "remove dependency all with package selector",
        &output,
        1,
    )
    .unwrap();
    expect_empty_stdout(
        "project-remove-dependency-all-package",
        "remove dependency all with package selector",
        &stdout,
    )
    .unwrap();
    expect_stderr_contains(
        "project-remove-dependency-all-package",
        "remove dependency all with package selector",
        &stderr,
        "error: `ql project remove-dependency --all` does not accept `--package`; bulk cleanup already targets all dependents of `--name`",
    )
    .unwrap();
}

#[test]
fn project_remove_dependency_all_requires_name_for_workspace_root() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-remove-dependency-all-derived-name-missing");
    let project_root = temp.path().join("workspace");

    temp.write(
        "workspace/qlang.toml",
        "[workspace]\nmembers = [\"packages/app\", \"packages/core\"]\n",
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        "[package]\nname = \"app\"\n\n[dependencies]\ncore = \"../core\"\n",
    );
    temp.write(
        "workspace/packages/core/qlang.toml",
        "[package]\nname = \"core\"\n",
    );

    let mut remove_dependency = ql_command(&workspace_root);
    remove_dependency.args([
        "project",
        "remove-dependency",
        &project_root.to_string_lossy(),
        "--all",
    ]);
    let output = run_command_capture(
        &mut remove_dependency,
        "`ql project remove-dependency --all` ambiguous workspace root",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-remove-dependency-all-derived-name-missing",
        "remove dependency from all workspace dependents with ambiguous workspace root",
        &output,
        1,
    )
    .unwrap();
    expect_empty_stdout(
        "project-remove-dependency-all-derived-name-missing",
        "remove dependency from all workspace dependents with ambiguous workspace root",
        &stdout,
    )
    .unwrap();
    expect_stderr_contains(
        "project-remove-dependency-all-derived-name-missing",
        "remove dependency from all workspace dependents with ambiguous workspace root",
        &stderr.replace('\\', "/"),
        &format!(
            "error: `ql project remove-dependency --all` could not derive a package name from `{}`; rerun with `--name <package>`",
            project_root.to_string_lossy().replace('\\', "/")
        ),
    )
    .unwrap();
}

#[test]
fn project_remove_dependency_all_refuses_workspace_package_without_dependents() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-remove-dependency-all-empty");
    let project_root = temp.path().join("workspace");

    temp.write(
        "workspace/qlang.toml",
        "[workspace]\nmembers = [\"packages/app\", \"packages/core\"]\n",
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        "[package]\nname = \"app\"\n",
    );
    temp.write(
        "workspace/packages/core/qlang.toml",
        "[package]\nname = \"core\"\n",
    );

    let mut remove_dependency = ql_command(&workspace_root);
    remove_dependency.args([
        "project",
        "remove-dependency",
        &project_root.to_string_lossy(),
        "--name",
        "core",
        "--all",
    ]);
    let output = run_command_capture(
        &mut remove_dependency,
        "`ql project remove-dependency --all` package without dependents",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-remove-dependency-all-empty",
        "remove dependency from workspace package without dependents",
        &output,
        1,
    )
    .unwrap();
    expect_empty_stdout(
        "project-remove-dependency-all-empty",
        "remove dependency from workspace package without dependents",
        &stdout,
    )
    .unwrap();
    expect_stderr_contains(
        "project-remove-dependency-all-empty",
        "remove dependency from workspace package without dependents",
        &stderr.replace('\\', "/"),
        &format!(
            "error: `ql project remove-dependency` workspace package `core` does not have any dependent members to update in workspace manifest `{}`",
            project_root.join("qlang.toml").to_string_lossy().replace('\\', "/")
        ),
    )
    .unwrap();
}
