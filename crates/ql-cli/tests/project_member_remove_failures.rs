mod support;

use support::{
    TempDir, expect_empty_stderr, expect_empty_stdout, expect_exit_code, expect_stderr_contains,
    expect_success, ql_command, read_normalized_file, run_command_capture, workspace_root,
};

#[test]
fn project_remove_refuses_workspace_member_with_dependents() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-remove-dependent");
    let project_root = temp.path().join("workspace");
    let request_path = project_root.join("packages/core/src/main.ql");

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
        "`ql project init` workspace for dependent remove",
    );
    let (_stdout, stderr) = expect_success(
        "project-remove-dependent",
        "workspace init for dependent remove",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-remove-dependent",
        "workspace init for dependent remove",
        &stderr,
    )
    .unwrap();

    let mut add_core = ql_command(&workspace_root);
    add_core.args([
        "project",
        "add",
        &project_root.to_string_lossy(),
        "--name",
        "core",
    ]);
    let output = run_command_capture(&mut add_core, "`ql project add` core for dependent remove");
    let (_stdout, stderr) = expect_success(
        "project-remove-dependent",
        "workspace core add for dependent remove",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-remove-dependent",
        "workspace core add for dependent remove",
        &stderr,
    )
    .unwrap();

    let mut add_tools = ql_command(&workspace_root);
    add_tools.args([
        "project",
        "add",
        &project_root.to_string_lossy(),
        "--name",
        "tools",
        "--dependency",
        "core",
    ]);
    let output = run_command_capture(
        &mut add_tools,
        "`ql project add` tools for dependent remove",
    );
    let (_stdout, stderr) = expect_success(
        "project-remove-dependent",
        "workspace tools add for dependent remove",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-remove-dependent",
        "workspace tools add for dependent remove",
        &stderr,
    )
    .unwrap();

    let mut remove = ql_command(&workspace_root);
    remove.args([
        "project",
        "remove",
        &request_path.to_string_lossy(),
        "--name",
        "core",
    ]);
    let output = run_command_capture(
        &mut remove,
        "`ql project remove` workspace member with dependents",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-remove-dependent",
        "remove workspace member with dependents",
        &output,
        1,
    )
    .unwrap();
    expect_empty_stdout(
        "project-remove-dependent",
        "remove workspace member with dependents",
        &stdout,
    )
    .unwrap();
    expect_stderr_contains(
        "project-remove-dependent",
        "remove workspace member with dependents",
        &stderr.replace('\\', "/"),
        &format!(
            "error: `ql project remove` cannot remove member package `core` from workspace manifest `{}` because other members still depend on it: packages/tools (tools); remove those edges first with `ql project remove-dependency <member> --name core` or rerun with `ql project remove <file-or-dir> --name core --cascade`",
            project_root.join("qlang.toml").to_string_lossy().replace('\\', "/")
        ),
    )
    .unwrap();

    let workspace_manifest = read_normalized_file(
        &project_root.join("qlang.toml"),
        "workspace manifest after refused dependent remove",
    );
    assert!(
        workspace_manifest.contains("packages/core"),
        "workspace manifest should keep dependent member after refused remove"
    );
    assert!(
        project_root.join("packages/core").is_dir(),
        "refused remove should keep member directory on disk"
    );
}

#[test]
fn project_remove_refuses_unknown_workspace_member_package() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-remove-missing");
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
    let output = run_command_capture(&mut init, "`ql project init` workspace for missing remove");
    let (_stdout, stderr) = expect_success(
        "project-remove-missing",
        "workspace init for missing remove",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-remove-missing",
        "workspace init for missing remove",
        &stderr,
    )
    .unwrap();

    let mut remove = ql_command(&workspace_root);
    remove.args([
        "project",
        "remove",
        &project_root.to_string_lossy(),
        "--name",
        "tools",
    ]);
    let output = run_command_capture(&mut remove, "`ql project remove` missing package");
    let (stdout, stderr) = expect_exit_code(
        "project-remove-missing",
        "remove missing workspace member package",
        &output,
        1,
    )
    .unwrap();
    expect_empty_stdout(
        "project-remove-missing",
        "remove missing workspace member package",
        &stdout,
    )
    .unwrap();
    expect_stderr_contains(
        "project-remove-missing",
        "remove missing workspace member package",
        &stderr.replace('\\', "/"),
        &format!(
            "error: `ql project remove` workspace manifest `{}` does not contain member package `tools`",
            project_root.join("qlang.toml").to_string_lossy().replace('\\', "/")
        ),
    )
    .unwrap();
}

#[test]
fn project_remove_rejects_ambiguous_workspace_member_package_names() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-remove-ambiguous");
    let project_root = temp.path().join("workspace");

    temp.write(
        "workspace/qlang.toml",
        "[workspace]\nmembers = [\"packages/a\", \"packages/b\"]\n",
    );
    temp.write(
        "workspace/packages/a/qlang.toml",
        "[package]\nname = \"util\"\n",
    );
    temp.write(
        "workspace/packages/b/qlang.toml",
        "[package]\nname = \"util\"\n",
    );

    let mut remove = ql_command(&workspace_root);
    remove.args([
        "project",
        "remove",
        &project_root.to_string_lossy(),
        "--name",
        "util",
    ]);
    let output = run_command_capture(&mut remove, "`ql project remove` ambiguous package");
    let (stdout, stderr) = expect_exit_code(
        "project-remove-ambiguous",
        "remove ambiguous workspace member package",
        &output,
        1,
    )
    .unwrap();
    expect_empty_stdout(
        "project-remove-ambiguous",
        "remove ambiguous workspace member package",
        &stdout,
    )
    .unwrap();
    expect_stderr_contains(
        "project-remove-ambiguous",
        "remove ambiguous workspace member package",
        &stderr.replace('\\', "/"),
        &format!(
            "error: `ql project remove` workspace manifest `{}` contains multiple members for package `util`: packages/a, packages/b",
            project_root.join("qlang.toml").to_string_lossy().replace('\\', "/")
        ),
    )
    .unwrap();
}
