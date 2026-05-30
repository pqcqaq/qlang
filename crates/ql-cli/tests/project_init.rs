mod support;

use std::process::Stdio;

use ql_driver::{ToolchainOptions, discover_toolchain};
use support::{
    TempDir, assert_no_build_lock_directories, executable_output_path, expect_empty_stderr,
    expect_empty_stdout, expect_exit_code, expect_file_exists, expect_silent_output,
    expect_stderr_contains, expect_stdout_contains_all, expect_success, ql_command,
    read_normalized_file, run_command_capture, workspace_root,
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
            &format!(
                "created: {}",
                project_root
                    .join("src")
                    .join("main.ql")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            &format!(
                "created: {}",
                project_root
                    .join("tests")
                    .join("smoke.ql")
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
    assert_eq!(
        read_normalized_file(&project_root.join("src/main.ql"), "package main source"),
        "fn main() -> Int {\n    return 0\n}\n"
    );
    assert_eq!(
        read_normalized_file(&project_root.join("tests/smoke.ql"), "package smoke test"),
        "fn main() -> Int {\n    return 0\n}\n"
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
fn project_init_creates_runnable_package_scaffold() {
    if !toolchain_available("`ql project init` runnable package test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-init-package-run");
    let project_root = temp.path().join("demo-package");
    let output_path = executable_output_path(&project_root.join("target/ql/debug"), "main");

    let mut init = ql_command(&workspace_root);
    init.args(["project", "init", &project_root.to_string_lossy()]);
    let output = run_command_capture(&mut init, "`ql project init` runnable package");
    let (_stdout, stderr) = expect_success(
        "project-init-package-run",
        "package init for runnable scaffold",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-package-run",
        "package init for runnable scaffold",
        &stderr,
    )
    .unwrap();

    let mut run = ql_command(&workspace_root);
    run.current_dir(temp.path());
    run.args(["run"]).arg(&project_root);
    let output = run_command_capture(&mut run, "`ql run` initialized package");
    let (stdout, stderr) = expect_exit_code(
        "project-init-package-run",
        "run initialized package",
        &output,
        0,
    )
    .unwrap();
    expect_silent_output(
        "project-init-package-run",
        "run initialized package",
        &stdout,
        &stderr,
    )
    .unwrap();
    expect_file_exists(
        "project-init-package-run",
        &output_path,
        "initialized package executable",
        "run initialized package",
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
                    .join("main.ql")
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
            &format!(
                "created: {}",
                project_root
                    .join("packages")
                    .join("app")
                    .join("tests")
                    .join("smoke.ql")
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
    assert_eq!(
        read_normalized_file(
            &project_root.join("packages/app/src/main.ql"),
            "workspace member main source"
        ),
        "fn main() -> Int {\n    return 0\n}\n"
    );
    assert_eq!(
        read_normalized_file(
            &project_root.join("packages/app/tests/smoke.ql"),
            "workspace member smoke test"
        ),
        "fn main() -> Int {\n    return 0\n}\n"
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

#[test]
fn project_init_refuses_existing_source_without_partial_manifest() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-init-source-conflict");
    let project_root = temp.path().join("demo-conflict");
    let existing_source = temp.write(
        "demo-conflict/src/main.ql",
        "fn old() -> Int { return 1 }\n",
    );

    let mut init = ql_command(&workspace_root);
    init.args(["project", "init", &project_root.to_string_lossy()]);
    let output = run_command_capture(&mut init, "`ql project init` conflicting source");
    let (stdout, stderr) = expect_exit_code(
        "project-init-source-conflict",
        "conflicting package source init",
        &output,
        1,
    )
    .unwrap();
    expect_empty_stdout(
        "project-init-source-conflict",
        "conflicting package source init",
        &stdout,
    )
    .unwrap();
    expect_stderr_contains(
        "project-init-source-conflict",
        "conflicting package source init",
        &stderr.replace('\\', "/"),
        &format!(
            "error: `ql project init` would overwrite existing path `{}`",
            existing_source.to_string_lossy().replace('\\', "/")
        ),
    )
    .unwrap();
    assert!(
        !project_root.join("qlang.toml").exists(),
        "package init should not leave a partial manifest when a later scaffold file conflicts"
    );
    assert!(
        !project_root.join("src/lib.ql").exists(),
        "package init should not create earlier scaffold files after preflight failure"
    );
    assert_eq!(
        read_normalized_file(&existing_source, "existing source after failed init"),
        "fn old() -> Int { return 1 }\n"
    );
}

#[test]
fn project_init_workspace_refuses_existing_member_source_without_partial_manifest() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-init-workspace-source-conflict");
    let project_root = temp.path().join("workspace");
    let existing_source = temp.write(
        "workspace/packages/app/src/lib.ql",
        "pub fn old() -> Int { return 1 }\n",
    );

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
        "`ql project init --workspace` conflicting source",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-init-workspace-source-conflict",
        "conflicting workspace member source init",
        &output,
        1,
    )
    .unwrap();
    expect_empty_stdout(
        "project-init-workspace-source-conflict",
        "conflicting workspace member source init",
        &stdout,
    )
    .unwrap();
    expect_stderr_contains(
        "project-init-workspace-source-conflict",
        "conflicting workspace member source init",
        &stderr.replace('\\', "/"),
        &format!(
            "error: `ql project init` would overwrite existing path `{}`",
            existing_source.to_string_lossy().replace('\\', "/")
        ),
    )
    .unwrap();
    assert!(
        !project_root.join("qlang.toml").exists(),
        "workspace init should not leave a partial workspace manifest when a member file conflicts"
    );
    assert!(
        !project_root.join("packages/app/qlang.toml").exists(),
        "workspace init should not create a partial member manifest after preflight failure"
    );
    assert!(
        !project_root.join("packages/app/src/main.ql").exists(),
        "workspace init should not create sibling source files after preflight failure"
    );
    assert_eq!(
        read_normalized_file(
            &existing_source,
            "existing member source after failed workspace init"
        ),
        "pub fn old() -> Int { return 1 }\n"
    );
}

#[test]
fn project_init_serializes_concurrent_scaffold_creation() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-init-concurrent-create");
    let project_root = temp.path().join("demo-concurrent");
    let manifest_path = project_root.join("qlang.toml");

    let mut children = Vec::new();
    for index in 0..4 {
        let mut init = ql_command(&workspace_root);
        init.current_dir(temp.path());
        init.args(["project", "init", &project_root.to_string_lossy()]);
        init.stdout(Stdio::piped()).stderr(Stdio::piped());
        let child = init
            .spawn()
            .unwrap_or_else(|error| panic!("spawn concurrent ql project init #{index}: {error}"));
        children.push((index, child));
    }

    let mut success_count = 0;
    let mut conflict_count = 0;
    for (index, child) in children {
        let output = child.wait_with_output().unwrap_or_else(|error| {
            panic!("wait for concurrent ql project init #{index}: {error}")
        });
        if output.status.success() {
            success_count += 1;
            let (stdout, stderr) = expect_success(
                "project-init-concurrent-create",
                &format!("concurrent package init #{index}"),
                &output,
            )
            .unwrap_or_else(|message| panic!("{message}"));
            expect_empty_stderr(
                "project-init-concurrent-create",
                &format!("concurrent package init #{index}"),
                &stderr,
            )
            .unwrap_or_else(|message| panic!("{message}"));
            expect_stdout_contains_all(
                "project-init-concurrent-create",
                &stdout.replace('\\', "/"),
                &[&format!(
                    "created: {}",
                    manifest_path.to_string_lossy().replace('\\', "/")
                )],
            )
            .unwrap_or_else(|message| panic!("{message}"));
        } else {
            conflict_count += 1;
            let (stdout, stderr) = expect_exit_code(
                "project-init-concurrent-create",
                &format!("concurrent package init #{index}"),
                &output,
                1,
            )
            .unwrap_or_else(|message| panic!("{message}"));
            expect_empty_stdout(
                "project-init-concurrent-create",
                &format!("concurrent package init #{index}"),
                &stdout,
            )
            .unwrap_or_else(|message| panic!("{message}"));
            expect_stderr_contains(
                "project-init-concurrent-create",
                &format!("concurrent package init #{index}"),
                &stderr.replace('\\', "/"),
                &format!(
                    "error: `ql project init` would overwrite existing path `{}`",
                    manifest_path.to_string_lossy().replace('\\', "/")
                ),
            )
            .unwrap_or_else(|message| panic!("{message}"));
        }
    }

    assert_eq!(
        success_count, 1,
        "exactly one concurrent project init should create the scaffold"
    );
    assert_eq!(
        conflict_count, 3,
        "remaining concurrent project init commands should refuse overwrite"
    );
    assert_eq!(
        read_normalized_file(&manifest_path, "concurrent package manifest"),
        "[package]\nname = \"demo-concurrent\"\n"
    );
    expect_file_exists(
        "project-init-concurrent-create",
        &project_root.join("src/lib.ql"),
        "concurrent package lib source",
        "concurrent package init",
    )
    .unwrap();
    expect_file_exists(
        "project-init-concurrent-create",
        &project_root.join("src/main.ql"),
        "concurrent package main source",
        "concurrent package init",
    )
    .unwrap();
    expect_file_exists(
        "project-init-concurrent-create",
        &project_root.join("tests/smoke.ql"),
        "concurrent package smoke test",
        "concurrent package init",
    )
    .unwrap();
    assert_no_build_lock_directories("project-init-concurrent-create", &project_root);
}
