mod support;

use support::project_init_stdlib::{
    assert_stdlib_check_json, assert_stdlib_dependencies_json, assert_stdlib_workspace_graph_json,
    assert_stdlib_workspace_status_json, expect_emit_interface_check_ok,
    expect_stdlib_starter_interface, expect_stdlib_starter_main_source,
    expect_stdlib_starter_smoke_source, expect_stdlib_starter_source, parse_json_output,
    write_repo_stdlib_fixture,
};
use support::{
    TempDir, expect_empty_stderr, expect_stdout_contains_all, expect_success, ql_command,
    read_normalized_file, run_command_capture, workspace_root,
};

#[test]
fn project_init_with_stdlib_creates_consuming_workspace_scaffold_and_check_succeeds() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-init-stdlib-workspace");
    let stdlib_root = write_repo_stdlib_fixture(&temp, &workspace_root);
    let project_root = temp.path().join("demo-workspace");
    let member_root = project_root.join("packages/app");

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
    let output = run_command_capture(&mut init, "`ql project init --workspace --stdlib`");
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-workspace",
        "stdlib workspace init",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-workspace",
        "stdlib workspace init",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "project-init-stdlib-workspace",
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
                member_root
                    .join("qlang.toml")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            &format!(
                "created: {}",
                member_root
                    .join("tests/smoke.ql")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
        ],
    )
    .unwrap();

    assert_eq!(
        read_normalized_file(
            &project_root.join("qlang.toml"),
            "stdlib workspace manifest"
        ),
        "[workspace]\nmembers = [\"packages/app\"]\n"
    );
    assert_eq!(
        read_normalized_file(
            &member_root.join("qlang.toml"),
            "stdlib workspace member manifest"
        ),
        "[package]\nname = \"app\"\n\n[dependencies]\n\"std.core\" = \"../../../stdlib/packages/core\"\n\"std.option\" = \"../../../stdlib/packages/option\"\n\"std.result\" = \"../../../stdlib/packages/result\"\n\"std.array\" = \"../../../stdlib/packages/array\"\n\"std.test\" = \"../../../stdlib/packages/test\"\n"
    );
    let lib_source = read_normalized_file(
        &member_root.join("src/lib.ql"),
        "stdlib workspace member source",
    );
    expect_stdlib_starter_source(&lib_source, "stdlib workspace member source");
    let main_source = read_normalized_file(
        &member_root.join("src/main.ql"),
        "stdlib workspace member main source",
    );
    expect_stdlib_starter_main_source(&main_source, "stdlib workspace member main source");
    let smoke_source = read_normalized_file(
        &member_root.join("tests/smoke.ql"),
        "stdlib workspace member smoke test",
    );
    expect_stdlib_starter_smoke_source(&smoke_source, "stdlib workspace member smoke test");

    let mut check = ql_command(&workspace_root);
    check.args(["check", &project_root.to_string_lossy()]);
    let output = run_command_capture(&mut check, "`ql check` initialized stdlib workspace");
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-workspace",
        "check initialized stdlib workspace",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-workspace",
        "check initialized stdlib workspace",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "project-init-stdlib-workspace",
        &stdout.replace('\\', "/"),
        &[
            &format!(
                "ok: {}",
                member_root
                    .join("src/lib.ql")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            "loaded interface:",
        ],
    )
    .unwrap();

    let mut check_json = ql_command(&workspace_root);
    check_json.args(["check", &project_root.to_string_lossy(), "--json"]);
    let output = run_command_capture(
        &mut check_json,
        "`ql check --json` initialized stdlib workspace",
    );
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-workspace",
        "json check initialized stdlib workspace",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-workspace",
        "json check initialized stdlib workspace",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("project-init-stdlib-workspace", &stdout);
    assert_stdlib_check_json(
        "initialized stdlib workspace check json",
        &actual,
        "workspace",
        &project_root.join("qlang.toml"),
        &[
            member_root.join("src/lib.ql"),
            member_root.join("src/main.ql"),
        ],
        &stdlib_root,
    );

    let member_interface = member_root.join("app.qi");
    let mut emit_interface = ql_command(&workspace_root);
    emit_interface.args([
        "project",
        "emit-interface",
        &project_root.to_string_lossy(),
        "--package",
        "app",
    ]);
    let output = run_command_capture(
        &mut emit_interface,
        "`ql project emit-interface --package app` initialized stdlib workspace",
    );
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-workspace",
        "emit interface initialized stdlib workspace package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-workspace",
        "emit interface initialized stdlib workspace package",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "project-init-stdlib-workspace",
        &stdout.replace('\\', "/"),
        &[&format!(
            "wrote interface: {}",
            member_interface.display().to_string().replace('\\', "/")
        )],
    )
    .unwrap();
    let interface_source = read_normalized_file(
        &member_interface,
        "initialized stdlib workspace member interface artifact",
    );
    expect_stdlib_starter_interface(
        &interface_source,
        "app",
        "initialized stdlib workspace member interface artifact",
    );
    expect_emit_interface_check_ok(
        "project-init-stdlib-workspace",
        &workspace_root,
        &project_root,
        Some("app"),
        &member_interface,
        "`ql project emit-interface --check --package app` initialized stdlib workspace",
    );

    let mut graph_json = ql_command(&workspace_root);
    graph_json.args([
        "project",
        "graph",
        &project_root.to_string_lossy(),
        "--package",
        "app",
        "--json",
    ]);
    let output = run_command_capture(
        &mut graph_json,
        "`ql project graph --json --package app` initialized stdlib workspace",
    );
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-workspace",
        "graph json initialized stdlib workspace package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-workspace",
        "graph json initialized stdlib workspace package",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("project-init-stdlib-workspace", &stdout);
    assert_stdlib_workspace_graph_json(
        "initialized stdlib workspace graph json",
        &actual,
        &member_root.join("qlang.toml"),
    );

    let mut status_json = ql_command(&workspace_root);
    status_json.args([
        "project",
        "status",
        &project_root.to_string_lossy(),
        "--package",
        "app",
        "--json",
    ]);
    let output = run_command_capture(
        &mut status_json,
        "`ql project status --json --package app` initialized stdlib workspace",
    );
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-workspace",
        "status json initialized stdlib workspace package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-workspace",
        "status json initialized stdlib workspace package",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("project-init-stdlib-workspace", &stdout);
    assert_stdlib_workspace_status_json(
        "initialized stdlib workspace status json",
        &actual,
        &project_root,
        &member_root,
    );

    let mut dependencies = ql_command(&workspace_root);
    dependencies.args([
        "project",
        "dependencies",
        &project_root.to_string_lossy(),
        "--name",
        "app",
    ]);
    let output = run_command_capture(
        &mut dependencies,
        "`ql project dependencies --name app` initialized stdlib workspace",
    );
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-workspace",
        "dependencies initialized stdlib workspace package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-workspace",
        "dependencies initialized stdlib workspace package",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "project-init-stdlib-workspace",
        &stdout.replace('\\', "/"),
        &[
            &format!(
                "workspace_manifest: {}",
                project_root
                    .join("qlang.toml")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            "package: app",
            "dependencies:",
            "  - ../../../stdlib/packages/core (std.core, local)",
            "  - ../../../stdlib/packages/option (std.option, local)",
            "  - ../../../stdlib/packages/result (std.result, local)",
            "  - ../../../stdlib/packages/array (std.array, local)",
            "  - ../../../stdlib/packages/test (std.test, local)",
        ],
    )
    .unwrap();

    let mut dependencies_json = ql_command(&workspace_root);
    dependencies_json.args([
        "project",
        "dependencies",
        &project_root.to_string_lossy(),
        "--name",
        "app",
        "--json",
    ]);
    let output = run_command_capture(
        &mut dependencies_json,
        "`ql project dependencies --json --name app` initialized stdlib workspace",
    );
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-workspace",
        "dependencies json initialized stdlib workspace package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-workspace",
        "dependencies json initialized stdlib workspace package",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("project-init-stdlib-workspace", &stdout);
    assert_stdlib_dependencies_json(
        "initialized stdlib workspace dependencies json",
        &actual,
        &project_root,
        &project_root.join("qlang.toml"),
        "app",
        "../../../stdlib/packages",
        &stdlib_root,
    );
}
