mod support;

use support::{
    TempDir, expect_empty_stderr, expect_empty_stdout, expect_stderr_contains,
    expect_stdout_contains_all, expect_success, ql_command, read_normalized_file,
    run_command_capture, workspace_root,
};

#[test]
fn project_init_creates_package_scaffold_and_check_succeeds() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-init-package");
    let project_root = temp.path().join("demo-package");

    let mut init = ql_command(&workspace_root);
    init.args(["project", "init", &project_root.to_string_lossy()]);
    let output = run_command_capture(&mut init, "`ql project init` package");
    let (stdout, stderr) = expect_success("project-init-package", "package init", &output).unwrap();
    expect_empty_stderr("project-init-package", "package init", &stderr).unwrap();
    expect_stdout_contains_all(
        "project-init-package",
        &stdout,
        &[
            &format!(
                "created: {}",
                project_root
                    .join("qlang.toml")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            &format!(
                "created: {}",
                project_root
                    .join("src")
                    .join("lib.ql")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
        ],
    )
    .unwrap();

    assert_eq!(
        read_normalized_file(&project_root.join("qlang.toml"), "package manifest"),
        "[package]\nname = \"demo-package\"\n"
    );
    assert_eq!(
        read_normalized_file(&project_root.join("src/lib.ql"), "package source"),
        "pub fn run() -> Int {\n    return 0\n}\n"
    );

    let mut check = ql_command(&workspace_root);
    check.args(["check", &project_root.to_string_lossy()]);
    let output = run_command_capture(&mut check, "`ql check` initialized package");
    let (stdout, stderr) =
        expect_success("project-init-package", "check initialized package", &output).unwrap();
    expect_empty_stderr("project-init-package", "check initialized package", &stderr).unwrap();
    expect_stdout_contains_all(
        "project-init-package",
        &stdout,
        &[&format!(
            "ok: {}",
            project_root.join("src").join("lib.ql").to_string_lossy()
        )],
    )
    .unwrap();
}

#[test]
fn project_init_creates_workspace_scaffold_and_graph_succeeds() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-init-workspace");
    let project_root = temp.path().join("demo-workspace");

    let mut init = ql_command(&workspace_root);
    init.args([
        "project",
        "init",
        &project_root.to_string_lossy(),
        "--workspace",
        "--name",
        "app",
    ]);
    let output = run_command_capture(&mut init, "`ql project init --workspace`");
    let (stdout, stderr) =
        expect_success("project-init-workspace", "workspace init", &output).unwrap();
    expect_empty_stderr("project-init-workspace", "workspace init", &stderr).unwrap();
    expect_stdout_contains_all(
        "project-init-workspace",
        &stdout,
        &[
            &format!(
                "created: {}",
                project_root
                    .join("qlang.toml")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            &format!(
                "created: {}",
                project_root
                    .join("packages")
                    .join("app")
                    .join("qlang.toml")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            &format!(
                "created: {}",
                project_root
                    .join("packages")
                    .join("app")
                    .join("src")
                    .join("lib.ql")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
        ],
    )
    .unwrap();

    assert_eq!(
        read_normalized_file(&project_root.join("qlang.toml"), "workspace manifest"),
        "[workspace]\nmembers = [\"packages/app\"]\n"
    );
    assert_eq!(
        read_normalized_file(
            &project_root.join("packages/app/qlang.toml"),
            "workspace member manifest"
        ),
        "[package]\nname = \"app\"\n"
    );

    let mut graph = ql_command(&workspace_root);
    graph.args(["project", "graph", &project_root.to_string_lossy()]);
    let output = run_command_capture(&mut graph, "`ql project graph` initialized workspace");
    let (stdout, stderr) = expect_success(
        "project-init-workspace",
        "graph initialized workspace",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-workspace",
        "graph initialized workspace",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "project-init-workspace",
        &stdout,
        &[
            "package: <none>",
            "workspace_members:",
            "  - packages/app",
            "workspace_packages:",
            "  - member: packages/app",
            "    package: app",
            "    status: missing",
        ],
    )
    .unwrap();
}

#[test]
fn project_init_refuses_to_overwrite_existing_manifest() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-init-conflict");
    let project_root = temp.path().join("demo-conflict");
    temp.write(
        "demo-conflict/qlang.toml",
        "[package]\nname = \"already-there\"\n",
    );

    let mut init = ql_command(&workspace_root);
    init.args(["project", "init", &project_root.to_string_lossy()]);
    let output = run_command_capture(&mut init, "`ql project init` conflicting manifest");
    let (stdout, stderr) = support::expect_exit_code(
        "project-init-conflict",
        "conflicting package init",
        &output,
        1,
    )
    .unwrap();
    expect_empty_stdout("project-init-conflict", "conflicting package init", &stdout).unwrap();
    expect_stderr_contains(
        "project-init-conflict",
        "conflicting package init",
        &stderr,
        &format!(
            "error: `ql project init` would overwrite existing path `{}`",
            project_root
                .join("qlang.toml")
                .to_string_lossy()
                .replace('\\', "/")
        ),
    )
    .unwrap();
}
