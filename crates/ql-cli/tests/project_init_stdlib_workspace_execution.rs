mod support;

use std::path::Path;

use support::project_init_stdlib::{
    assert_repo_stdlib_build_json, assert_repo_stdlib_run_json,
    assert_repo_stdlib_starter_build_json, assert_repo_stdlib_starter_check_json,
    assert_repo_stdlib_starter_test_json, parse_json_output, toolchain_available,
};
use support::{
    executable_output_path, expect_empty_stderr, expect_file_exists, expect_success, ql_command,
    run_command_capture, static_library_output_path, workspace_root,
};

#[test]
fn repo_stdlib_workspace_checks_builds_and_tests_starter_package() {
    if !toolchain_available("`ql check/build/test --package` repo stdlib starter") {
        return;
    }

    let workspace_root = workspace_root();

    let mut check = ql_command(&workspace_root);
    check.args(["check", "stdlib", "--package", "stdlib.starter", "--json"]);
    let output = run_command_capture(
        &mut check,
        "`ql check stdlib --package stdlib.starter --json`",
    );
    let (stdout, stderr) = expect_success(
        "repo-stdlib-workspace-starter-package",
        "check repo stdlib starter package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "repo-stdlib-workspace-starter-package",
        "check repo stdlib starter package",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("repo-stdlib-workspace-starter-package", &stdout);
    assert_repo_stdlib_starter_check_json(
        "repo stdlib starter check json",
        &actual,
        Path::new("stdlib"),
    );

    let mut build = ql_command(&workspace_root);
    build.args(["build", "stdlib", "--package", "stdlib.starter", "--json"]);
    let output = run_command_capture(
        &mut build,
        "`ql build stdlib --package stdlib.starter --json`",
    );
    let (stdout, stderr) = expect_success(
        "repo-stdlib-workspace-starter-package",
        "build repo stdlib starter package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "repo-stdlib-workspace-starter-package",
        "build repo stdlib starter package",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("repo-stdlib-workspace-starter-package", &stdout);
    assert_repo_stdlib_starter_build_json(
        "repo stdlib starter build json",
        &actual,
        Path::new("stdlib"),
    );

    let mut test = ql_command(&workspace_root);
    test.args(["test", "stdlib", "--package", "stdlib.starter", "--json"]);
    let output = run_command_capture(
        &mut test,
        "`ql test stdlib --package stdlib.starter --json`",
    );
    let (stdout, stderr) = expect_success(
        "repo-stdlib-workspace-starter-package",
        "test repo stdlib starter package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "repo-stdlib-workspace-starter-package",
        "test repo stdlib starter package",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("repo-stdlib-workspace-starter-package", &stdout);
    assert_repo_stdlib_starter_test_json(
        "repo stdlib starter test json",
        &actual,
        Path::new("stdlib"),
    );
}

#[test]
fn repo_stdlib_workspace_builds_and_runs_starter() {
    if !toolchain_available("`ql build/run` repo stdlib workspace") {
        return;
    }

    let workspace_root = workspace_root();

    let mut build = ql_command(&workspace_root);
    build.args(["build", "stdlib", "--json"]);
    let output = run_command_capture(&mut build, "`ql build stdlib --json`");
    let (stdout, stderr) = expect_success(
        "repo-stdlib-workspace-build-run",
        "build repo stdlib workspace",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "repo-stdlib-workspace-build-run",
        "build repo stdlib workspace",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("repo-stdlib-workspace-build-run", &stdout);
    assert_repo_stdlib_build_json(
        "repo stdlib workspace build json",
        &actual,
        Path::new("stdlib"),
    );
    for (context, path) in [
        (
            "std.core static library",
            static_library_output_path(
                &workspace_root.join("stdlib/packages/core/target/ql/debug"),
                "lib",
            ),
        ),
        (
            "std.option static library",
            static_library_output_path(
                &workspace_root.join("stdlib/packages/option/target/ql/debug"),
                "lib",
            ),
        ),
        (
            "std.result static library",
            static_library_output_path(
                &workspace_root.join("stdlib/packages/result/target/ql/debug"),
                "lib",
            ),
        ),
        (
            "std.array static library",
            static_library_output_path(
                &workspace_root.join("stdlib/packages/array/target/ql/debug"),
                "lib",
            ),
        ),
        (
            "std.test static library",
            static_library_output_path(
                &workspace_root.join("stdlib/packages/test/target/ql/debug"),
                "lib",
            ),
        ),
        (
            "stdlib starter static library",
            static_library_output_path(
                &workspace_root.join("stdlib/examples/starter/target/ql/debug"),
                "lib",
            ),
        ),
        (
            "stdlib starter llvm-ir",
            workspace_root.join("stdlib/examples/starter/target/ql/debug/main.ll"),
        ),
    ] {
        expect_file_exists(
            "repo-stdlib-workspace-build-run",
            &path,
            context,
            "`ql build stdlib --json`",
        )
        .unwrap();
    }

    let mut run = ql_command(&workspace_root);
    run.args(["run", "stdlib", "--package", "stdlib.starter", "--json"]);
    let output = run_command_capture(&mut run, "`ql run stdlib --package stdlib.starter --json`");
    let (stdout, stderr) = expect_success(
        "repo-stdlib-workspace-build-run",
        "run repo stdlib starter",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "repo-stdlib-workspace-build-run",
        "run repo stdlib starter",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("repo-stdlib-workspace-build-run", &stdout);
    assert_repo_stdlib_run_json("repo stdlib starter run json", &actual, Path::new("stdlib"));
    expect_file_exists(
        "repo-stdlib-workspace-build-run",
        &executable_output_path(
            &workspace_root.join("stdlib/examples/starter/target/ql/debug"),
            "main",
        ),
        "stdlib starter executable",
        "`ql run stdlib --package stdlib.starter --json`",
    )
    .unwrap();
}
