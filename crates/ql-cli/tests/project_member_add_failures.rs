mod support;

use support::{
    TempDir, expect_empty_stderr, expect_empty_stdout, expect_exit_code, expect_stderr_contains,
    expect_success, ql_command, run_command_capture, workspace_root,
};

#[test]
fn project_add_refuses_duplicate_workspace_package_name() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-add-duplicate");
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
    let output = run_command_capture(&mut init, "`ql project init` workspace for duplicate add");
    let (_stdout, stderr) = expect_success(
        "project-add-duplicate",
        "workspace init for duplicate add",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-add-duplicate",
        "workspace init for duplicate add",
        &stderr,
    )
    .unwrap();

    let mut add = ql_command(&workspace_root);
    add.args([
        "project",
        "add",
        &project_root.to_string_lossy(),
        "--name",
        "app",
    ]);
    let output = run_command_capture(&mut add, "`ql project add` duplicate package");
    let (stdout, stderr) = expect_exit_code(
        "project-add-duplicate",
        "duplicate workspace package add",
        &output,
        1,
    )
    .unwrap();
    expect_empty_stdout(
        "project-add-duplicate",
        "duplicate workspace package add",
        &stdout,
    )
    .unwrap();
    expect_stderr_contains(
        "project-add-duplicate",
        "duplicate workspace package add",
        &stderr,
        "already declares member `packages/app`",
    )
    .unwrap();
}

#[test]
fn project_add_refuses_to_overwrite_existing_member_directory() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-add-conflict");
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
    let output = run_command_capture(&mut init, "`ql project init` workspace for conflict add");
    let (_stdout, stderr) = expect_success(
        "project-add-conflict",
        "workspace init for conflict add",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-add-conflict",
        "workspace init for conflict add",
        &stderr,
    )
    .unwrap();

    temp.write("workspace/packages/tools/placeholder.txt", "already-here");

    let mut add = ql_command(&workspace_root);
    add.args([
        "project",
        "add",
        &project_root.to_string_lossy(),
        "--name",
        "tools",
    ]);
    let output = run_command_capture(&mut add, "`ql project add` conflicting member directory");
    let (stdout, stderr) = expect_exit_code(
        "project-add-conflict",
        "conflicting workspace member add",
        &output,
        1,
    )
    .unwrap();
    expect_empty_stdout(
        "project-add-conflict",
        "conflicting workspace member add",
        &stdout,
    )
    .unwrap();
    expect_stderr_contains(
        "project-add-conflict",
        "conflicting workspace member add",
        &stderr,
        &format!(
            "error: `ql project add` would overwrite existing path `{}`",
            project_root
                .join("packages/tools")
                .to_string_lossy()
                .replace('\\', "/")
        ),
    )
    .unwrap();
}

#[test]
fn project_add_rejects_ambiguous_existing_workspace_package_name() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-add-ambiguous-package");
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

    let mut add = ql_command(&workspace_root);
    add.args([
        "project",
        "add",
        &project_root.to_string_lossy(),
        "--name",
        "util",
    ]);
    let output = run_command_capture(&mut add, "`ql project add` ambiguous existing package");
    let (stdout, stderr) = expect_exit_code(
        "project-add-ambiguous-package",
        "add workspace member with ambiguous existing package name",
        &output,
        1,
    )
    .unwrap();
    expect_empty_stdout(
        "project-add-ambiguous-package",
        "add workspace member with ambiguous existing package name",
        &stdout,
    )
    .unwrap();
    expect_stderr_contains(
        "project-add-ambiguous-package",
        "add workspace member with ambiguous existing package name",
        &stderr.replace('\\', "/"),
        &format!(
            "error: `ql project add` workspace manifest `{}` contains multiple members for package `util`: packages/a, packages/b",
            project_root.join("qlang.toml").to_string_lossy().replace('\\', "/")
        ),
    )
    .unwrap();
    assert!(
        !project_root.join("packages/util").exists(),
        "ambiguous package add should not create the new workspace member directory"
    );
}

#[test]
fn project_add_existing_refuses_name_override() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-add-existing-name");
    let project_root = temp.path().join("workspace");
    let existing_member_root = project_root.join("vendor/core");

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
        "`ql project init` workspace for existing add name conflict",
    );
    let (_stdout, stderr) = expect_success(
        "project-add-existing-name",
        "workspace init for existing add name conflict",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-add-existing-name",
        "workspace init for existing add name conflict",
        &stderr,
    )
    .unwrap();

    temp.write(
        "workspace/vendor/core/qlang.toml",
        "[package]\nname = \"core\"\n",
    );

    let mut add = ql_command(&workspace_root);
    add.args([
        "project",
        "add",
        &project_root.to_string_lossy(),
        "--existing",
        &existing_member_root.to_string_lossy(),
        "--name",
        "core",
    ]);
    let output = run_command_capture(&mut add, "`ql project add --existing --name`");
    let (stdout, stderr) = expect_exit_code(
        "project-add-existing-name",
        "add existing workspace member with explicit name override",
        &output,
        1,
    )
    .unwrap();
    expect_empty_stdout(
        "project-add-existing-name",
        "add existing workspace member with explicit name override",
        &stdout,
    )
    .unwrap();
    expect_stderr_contains(
        "project-add-existing-name",
        "add existing workspace member with explicit name override",
        &stderr,
        "error: `ql project add --existing` does not accept `--name`; package name comes from the existing manifest",
    )
    .unwrap();
}

#[test]
fn project_add_existing_rejects_ambiguous_existing_workspace_package_name() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-add-existing-ambiguous-package");
    let project_root = temp.path().join("workspace");
    let existing_member_root = project_root.join("vendor/util");

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
    temp.write(
        "workspace/vendor/util/qlang.toml",
        "[package]\nname = \"util\"\n",
    );

    let mut add = ql_command(&workspace_root);
    add.args([
        "project",
        "add",
        &project_root.to_string_lossy(),
        "--existing",
        &existing_member_root.to_string_lossy(),
    ]);
    let output = run_command_capture(
        &mut add,
        "`ql project add --existing` ambiguous existing package",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-add-existing-ambiguous-package",
        "add existing workspace member with ambiguous existing package name",
        &output,
        1,
    )
    .unwrap();
    expect_empty_stdout(
        "project-add-existing-ambiguous-package",
        "add existing workspace member with ambiguous existing package name",
        &stdout,
    )
    .unwrap();
    expect_stderr_contains(
        "project-add-existing-ambiguous-package",
        "add existing workspace member with ambiguous existing package name",
        &stderr.replace('\\', "/"),
        &format!(
            "error: `ql project add` workspace manifest `{}` contains multiple members for package `util`: packages/a, packages/b",
            project_root.join("qlang.toml").to_string_lossy().replace('\\', "/")
        ),
    )
    .unwrap();
}

#[test]
fn project_add_refuses_unknown_workspace_dependency() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-add-missing-dependency");
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
        "`ql project init` workspace for missing dependency add",
    );
    let (_stdout, stderr) = expect_success(
        "project-add-missing-dependency",
        "workspace init for missing dependency add",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-add-missing-dependency",
        "workspace init for missing dependency add",
        &stderr,
    )
    .unwrap();

    let mut add = ql_command(&workspace_root);
    add.args([
        "project",
        "add",
        &project_root.to_string_lossy(),
        "--name",
        "tools",
        "--dependency",
        "missing",
    ]);
    let output = run_command_capture(&mut add, "`ql project add` missing dependency");
    let (stdout, stderr) = expect_exit_code(
        "project-add-missing-dependency",
        "workspace member add with missing dependency",
        &output,
        1,
    )
    .unwrap();
    expect_empty_stdout(
        "project-add-missing-dependency",
        "workspace member add with missing dependency",
        &stdout,
    )
    .unwrap();
    expect_stderr_contains(
        "project-add-missing-dependency",
        "workspace member add with missing dependency",
        &stderr,
        &format!(
            "error: `ql project add` workspace manifest `{}` does not contain package `missing`",
            project_root
                .join("qlang.toml")
                .to_string_lossy()
                .replace('\\', "/")
        ),
    )
    .unwrap();
    assert!(
        !project_root.join("packages/tools").exists(),
        "missing dependency add should not create the new workspace member directory"
    );
}

#[test]
fn project_add_refuses_ambiguous_workspace_dependency_package() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-add-ambiguous-dependency");
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

    let mut add = ql_command(&workspace_root);
    add.args([
        "project",
        "add",
        &project_root.to_string_lossy(),
        "--name",
        "tools",
        "--dependency",
        "util",
    ]);
    let output = run_command_capture(&mut add, "`ql project add` ambiguous dependency");
    let (stdout, stderr) = expect_exit_code(
        "project-add-ambiguous-dependency",
        "workspace member add with ambiguous dependency package",
        &output,
        1,
    )
    .unwrap();
    expect_empty_stdout(
        "project-add-ambiguous-dependency",
        "workspace member add with ambiguous dependency package",
        &stdout,
    )
    .unwrap();
    expect_stderr_contains(
        "project-add-ambiguous-dependency",
        "workspace member add with ambiguous dependency package",
        &stderr.replace('\\', "/"),
        &format!(
            "error: `ql project add` workspace manifest `{}` contains multiple members for package `util`: packages/a, packages/b",
            project_root.join("qlang.toml").to_string_lossy().replace('\\', "/")
        ),
    )
    .unwrap();
    assert!(
        !project_root.join("packages/tools").exists(),
        "ambiguous dependency add should not create the new workspace member directory"
    );
}
