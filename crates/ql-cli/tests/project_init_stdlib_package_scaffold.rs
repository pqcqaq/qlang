mod support;

use support::project_init_stdlib::{
    assert_stdlib_check_json, assert_stdlib_dependencies_json, assert_stdlib_empty_dependents_json,
    assert_stdlib_graph_json, assert_stdlib_package_status_json, expect_emit_interface_check_ok,
    expect_stdlib_starter_interface, expect_stdlib_starter_main_source,
    expect_stdlib_starter_smoke_source, expect_stdlib_starter_source, parse_json_output,
    write_repo_stdlib_fixture,
};
use support::{
    TempDir, expect_empty_stderr, expect_stdout_contains_all, expect_success, ql_command,
    read_normalized_file, run_command_capture, workspace_root,
};

#[test]
fn project_init_with_stdlib_creates_consuming_package_scaffold_and_check_succeeds() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-init-stdlib-package");
    let stdlib_root = write_repo_stdlib_fixture(&temp, &workspace_root);
    let project_root = temp.path().join("demo-package");

    let mut init = ql_command(&workspace_root);
    init.args([
        "project",
        "init",
        &project_root.to_string_lossy(),
        "--stdlib",
        &stdlib_root.to_string_lossy(),
    ]);
    let output = run_command_capture(&mut init, "`ql project init --stdlib` package");
    let (_stdout, stderr) = expect_success(
        "project-init-stdlib-package",
        "stdlib package init",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-package",
        "stdlib package init",
        &stderr,
    )
    .unwrap();

    assert_eq!(
        read_normalized_file(&project_root.join("qlang.toml"), "stdlib package manifest"),
        "[package]\nname = \"demo-package\"\n\n[dependencies]\n\"std.core\" = \"../stdlib/packages/core\"\n\"std.option\" = \"../stdlib/packages/option\"\n\"std.result\" = \"../stdlib/packages/result\"\n\"std.array\" = \"../stdlib/packages/array\"\n\"std.test\" = \"../stdlib/packages/test\"\n"
    );
    let lib_source =
        read_normalized_file(&project_root.join("src/lib.ql"), "stdlib package source");
    expect_stdlib_starter_source(&lib_source, "stdlib package source");
    let main_source = read_normalized_file(
        &project_root.join("src/main.ql"),
        "stdlib package main source",
    );
    expect_stdlib_starter_main_source(&main_source, "stdlib package main source");
    let smoke_source = read_normalized_file(
        &project_root.join("tests/smoke.ql"),
        "stdlib package smoke test",
    );
    expect_stdlib_starter_smoke_source(&smoke_source, "stdlib package smoke test");

    let mut check = ql_command(&workspace_root);
    check.args(["check", &project_root.to_string_lossy()]);
    let output = run_command_capture(&mut check, "`ql check` initialized stdlib package");
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-package",
        "check initialized stdlib package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-package",
        "check initialized stdlib package",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "project-init-stdlib-package",
        &stdout.replace('\\', "/"),
        &[
            &format!(
                "ok: {}",
                project_root
                    .join("src")
                    .join("lib.ql")
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
        "`ql check --json` initialized stdlib package",
    );
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-package",
        "json check initialized stdlib package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-package",
        "json check initialized stdlib package",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("project-init-stdlib-package", &stdout);
    assert_stdlib_check_json(
        "initialized stdlib package check json",
        &actual,
        "package",
        &project_root.join("qlang.toml"),
        &[
            project_root.join("src/lib.ql"),
            project_root.join("src/main.ql"),
        ],
        &stdlib_root,
    );

    let package_interface = project_root.join("demo-package.qi");
    let mut emit_interface = ql_command(&workspace_root);
    emit_interface.args(["project", "emit-interface", &project_root.to_string_lossy()]);
    let output = run_command_capture(
        &mut emit_interface,
        "`ql project emit-interface` initialized stdlib package",
    );
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-package",
        "emit interface initialized stdlib package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-package",
        "emit interface initialized stdlib package",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "project-init-stdlib-package",
        &stdout.replace('\\', "/"),
        &[&format!(
            "wrote interface: {}",
            package_interface.display().to_string().replace('\\', "/")
        )],
    )
    .unwrap();
    let interface_source = read_normalized_file(
        &package_interface,
        "initialized stdlib package interface artifact",
    );
    expect_stdlib_starter_interface(
        &interface_source,
        "demo-package",
        "initialized stdlib package interface artifact",
    );
    expect_emit_interface_check_ok(
        "project-init-stdlib-package",
        &workspace_root,
        &project_root,
        None,
        &package_interface,
        "`ql project emit-interface --check` initialized stdlib package",
    );

    let mut graph_json = ql_command(&workspace_root);
    graph_json.args([
        "project",
        "graph",
        &project_root.to_string_lossy(),
        "--json",
    ]);
    let output = run_command_capture(
        &mut graph_json,
        "`ql project graph --json` initialized stdlib package",
    );
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-package",
        "graph json initialized stdlib package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-package",
        "graph json initialized stdlib package",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("project-init-stdlib-package", &stdout);
    assert_stdlib_graph_json(
        "initialized stdlib package graph json",
        &actual,
        "demo-package",
        &project_root.join("qlang.toml"),
        "demo-package.qi",
        "../stdlib/packages",
    );

    let mut status_json = ql_command(&workspace_root);
    status_json.args([
        "project",
        "status",
        &project_root.to_string_lossy(),
        "--json",
    ]);
    let output = run_command_capture(
        &mut status_json,
        "`ql project status --json` initialized stdlib package",
    );
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-package",
        "status json initialized stdlib package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-package",
        "status json initialized stdlib package",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("project-init-stdlib-package", &stdout);
    assert_stdlib_package_status_json(
        "initialized stdlib package status json",
        &actual,
        &project_root,
    );

    let mut dependencies = ql_command(&workspace_root);
    dependencies.args(["project", "dependencies", &project_root.to_string_lossy()]);
    let output = run_command_capture(
        &mut dependencies,
        "`ql project dependencies` initialized stdlib package",
    );
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-package",
        "dependencies initialized stdlib package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-package",
        "dependencies initialized stdlib package",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "project-init-stdlib-package",
        &stdout.replace('\\', "/"),
        &[
            &format!(
                "workspace_manifest: {}",
                project_root
                    .join("qlang.toml")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            "package: demo-package",
            "dependencies:",
            "  - ../stdlib/packages/core (std.core, local)",
            "  - ../stdlib/packages/option (std.option, local)",
            "  - ../stdlib/packages/result (std.result, local)",
            "  - ../stdlib/packages/array (std.array, local)",
            "  - ../stdlib/packages/test (std.test, local)",
        ],
    )
    .unwrap();

    let mut dependencies_json = ql_command(&workspace_root);
    dependencies_json.args([
        "project",
        "dependencies",
        &project_root.to_string_lossy(),
        "--json",
    ]);
    let output = run_command_capture(
        &mut dependencies_json,
        "`ql project dependencies --json` initialized stdlib package",
    );
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-package",
        "dependencies json initialized stdlib package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-package",
        "dependencies json initialized stdlib package",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("project-init-stdlib-package", &stdout);
    assert_stdlib_dependencies_json(
        "initialized stdlib package dependencies json",
        &actual,
        &project_root,
        &project_root.join("qlang.toml"),
        "demo-package",
        "../stdlib/packages",
        &stdlib_root,
    );

    let mut dependencies_selector_json = ql_command(&workspace_root);
    dependencies_selector_json.args([
        "project",
        "dependencies",
        &project_root.to_string_lossy(),
        "--name",
        "demo-package",
        "--json",
    ]);
    let output = run_command_capture(
        &mut dependencies_selector_json,
        "`ql project dependencies --json --name demo-package` initialized stdlib package",
    );
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-package",
        "dependencies json selector initialized stdlib package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-package",
        "dependencies json selector initialized stdlib package",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("project-init-stdlib-package", &stdout);
    assert_stdlib_dependencies_json(
        "initialized stdlib package dependencies selector json",
        &actual,
        &project_root,
        &project_root.join("qlang.toml"),
        "demo-package",
        "../stdlib/packages",
        &stdlib_root,
    );

    let mut dependents_selector_json = ql_command(&workspace_root);
    dependents_selector_json.args([
        "project",
        "dependents",
        &project_root.to_string_lossy(),
        "--name",
        "demo-package",
        "--json",
    ]);
    let output = run_command_capture(
        &mut dependents_selector_json,
        "`ql project dependents --json --name demo-package` initialized stdlib package",
    );
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-package",
        "dependents json selector initialized stdlib package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-package",
        "dependents json selector initialized stdlib package",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("project-init-stdlib-package", &stdout);
    assert_stdlib_empty_dependents_json(
        "initialized stdlib package dependents selector json",
        &actual,
        &project_root,
        &project_root.join("qlang.toml"),
        "demo-package",
    );
}
