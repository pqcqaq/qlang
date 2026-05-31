mod support;

use support::project_init_stdlib::{
    assert_repo_stdlib_run_json, assert_repo_stdlib_starter_build_json,
    assert_repo_stdlib_starter_test_json, parse_json_output, toolchain_available,
    write_repo_stdlib_fixture,
};
use support::{
    TempDir, executable_output_path, expect_empty_stderr, expect_file_exists, expect_success,
    ql_command, run_command_capture, static_library_output_path, workspace_root,
};

#[test]
fn repo_stdlib_fixture_builds_runs_and_tests_starter_package() {
    if !toolchain_available("`ql build/run/test --package` copied repo stdlib starter") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-repo-stdlib-workspace-starter-package");
    let stdlib_root = write_repo_stdlib_fixture(&temp, &workspace_root);

    let mut build = ql_command(&workspace_root);
    build
        .args(["build"])
        .arg(&stdlib_root)
        .args(["--package", "stdlib.starter", "--json"]);
    let output = run_command_capture(
        &mut build,
        "`ql build --package stdlib.starter --json` copied repo stdlib",
    );
    let (stdout, stderr) = expect_success(
        "repo-stdlib-workspace-starter-package-fixture",
        "build copied repo stdlib starter package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "repo-stdlib-workspace-starter-package-fixture",
        "build copied repo stdlib starter package",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("repo-stdlib-workspace-starter-package-fixture", &stdout);
    assert_repo_stdlib_starter_build_json(
        "copied repo stdlib starter build json",
        &actual,
        &stdlib_root,
    );

    for (context, path) in [
        (
            "copied stdlib starter static library",
            static_library_output_path(
                &stdlib_root.join("examples/starter/target/ql/debug"),
                "lib",
            ),
        ),
        (
            "copied stdlib starter llvm-ir",
            stdlib_root.join("examples/starter/target/ql/debug/main.ll"),
        ),
    ] {
        expect_file_exists(
            "repo-stdlib-workspace-starter-package-fixture",
            &path,
            context,
            "`ql build --package stdlib.starter --json` copied repo stdlib",
        )
        .unwrap();
    }

    let mut run = ql_command(&workspace_root);
    run.args(["run"])
        .arg(&stdlib_root)
        .args(["--package", "stdlib.starter", "--json"]);
    let output = run_command_capture(
        &mut run,
        "`ql run --package stdlib.starter --json` copied repo stdlib",
    );
    let (stdout, stderr) = expect_success(
        "repo-stdlib-workspace-starter-package-fixture",
        "run copied repo stdlib starter package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "repo-stdlib-workspace-starter-package-fixture",
        "run copied repo stdlib starter package",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("repo-stdlib-workspace-starter-package-fixture", &stdout);
    assert_repo_stdlib_run_json("copied repo stdlib starter run json", &actual, &stdlib_root);
    expect_file_exists(
        "repo-stdlib-workspace-starter-package-fixture",
        &executable_output_path(
            &stdlib_root.join("examples/starter/target/ql/debug"),
            "main",
        ),
        "copied stdlib starter executable",
        "`ql run --package stdlib.starter --json` copied repo stdlib",
    )
    .unwrap();

    let mut test = ql_command(&workspace_root);
    test.args(["test"])
        .arg(&stdlib_root)
        .args(["--package", "stdlib.starter", "--json"]);
    let output = run_command_capture(
        &mut test,
        "`ql test --package stdlib.starter --json` copied repo stdlib",
    );
    let (stdout, stderr) = expect_success(
        "repo-stdlib-workspace-starter-package-fixture",
        "test copied repo stdlib starter package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "repo-stdlib-workspace-starter-package-fixture",
        "test copied repo stdlib starter package",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("repo-stdlib-workspace-starter-package-fixture", &stdout);
    assert_repo_stdlib_starter_test_json(
        "copied repo stdlib starter test json",
        &actual,
        &stdlib_root,
    );
    expect_file_exists(
        "repo-stdlib-workspace-starter-package-fixture",
        &executable_output_path(
            &stdlib_root.join("examples/starter/target/ql/debug/tests"),
            "smoke",
        ),
        "copied stdlib starter smoke executable",
        "`ql test --package stdlib.starter --json` copied repo stdlib",
    )
    .unwrap();
}
