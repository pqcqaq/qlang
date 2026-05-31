mod support;

use serde_json::Value as JsonValue;
use support::project_init_stdlib::{
    json_path, parse_json_output, toolchain_available, write_repo_stdlib_fixture,
};
use support::{
    TempDir, executable_output_path, expect_empty_stderr, expect_file_exists,
    expect_stdout_contains_all, expect_success, ql_command, run_command_capture, workspace_root,
};

#[test]
fn repo_stdlib_fixture_runs_all_workspace_smoke_tests() {
    if !toolchain_available("`ql test` copied repo stdlib workspace") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-repo-stdlib-workspace-test");
    let stdlib_root = write_repo_stdlib_fixture(&temp, &workspace_root);

    let mut test = ql_command(&workspace_root);
    test.current_dir(temp.path());
    test.args(["test"]).arg(&stdlib_root);
    let output = run_command_capture(&mut test, "`ql test` copied repo stdlib workspace");
    let (stdout, stderr) = expect_success(
        "repo-stdlib-workspace-test",
        "copied repo stdlib workspace test",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "repo-stdlib-workspace-test",
        "copied repo stdlib workspace test",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "repo-stdlib-workspace-test",
        &stdout.replace('\\', "/"),
        &[
            "test packages/core/tests/smoke.ql ... ok",
            "test packages/option/tests/smoke.ql ... ok",
            "test packages/result/tests/smoke.ql ... ok",
            "test packages/array/tests/smoke.ql ... ok",
            "test packages/test/tests/smoke.ql ... ok",
            "test examples/starter/tests/smoke.ql ... ok",
            "test result: ok. 6 passed; 0 failed",
        ],
    )
    .unwrap();

    for (context, output_path) in [
        (
            "std.core smoke executable",
            executable_output_path(
                &stdlib_root.join("packages/core/target/ql/debug/tests"),
                "smoke",
            ),
        ),
        (
            "std.option smoke executable",
            executable_output_path(
                &stdlib_root.join("packages/option/target/ql/debug/tests"),
                "smoke",
            ),
        ),
        (
            "std.result smoke executable",
            executable_output_path(
                &stdlib_root.join("packages/result/target/ql/debug/tests"),
                "smoke",
            ),
        ),
        (
            "std.array smoke executable",
            executable_output_path(
                &stdlib_root.join("packages/array/target/ql/debug/tests"),
                "smoke",
            ),
        ),
        (
            "std.test smoke executable",
            executable_output_path(
                &stdlib_root.join("packages/test/target/ql/debug/tests"),
                "smoke",
            ),
        ),
        (
            "stdlib starter smoke executable",
            executable_output_path(
                &stdlib_root.join("examples/starter/target/ql/debug/tests"),
                "smoke",
            ),
        ),
    ] {
        expect_file_exists(
            "repo-stdlib-workspace-test",
            &output_path,
            context,
            "`ql test` copied repo stdlib workspace",
        )
        .unwrap();
    }

    let mut test_json = ql_command(&workspace_root);
    test_json.current_dir(temp.path());
    test_json.args(["test", "--json"]).arg(&stdlib_root);
    let output = run_command_capture(
        &mut test_json,
        "`ql test --json` copied repo stdlib workspace",
    );
    let (stdout, stderr) = expect_success(
        "repo-stdlib-workspace-test",
        "copied repo stdlib workspace json test",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "repo-stdlib-workspace-test",
        "copied repo stdlib workspace json test",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("repo-stdlib-workspace-test", &stdout);
    let expected = serde_json::json!({
        "schema": "ql.test.v1",
        "path": json_path(&stdlib_root),
        "requested_profile": "debug",
        "profile_overridden": false,
        "package_name": JsonValue::Null,
        "filter": JsonValue::Null,
        "list_only": false,
        "status": "ok",
        "discovered_total": 6,
        "selected_total": 6,
        "targets": [
            {
                "path": "packages/core/tests/smoke.ql",
                "kind": "smoke",
                "profile": "debug",
            },
            {
                "path": "packages/option/tests/smoke.ql",
                "kind": "smoke",
                "profile": "debug",
            },
            {
                "path": "packages/result/tests/smoke.ql",
                "kind": "smoke",
                "profile": "debug",
            },
            {
                "path": "packages/array/tests/smoke.ql",
                "kind": "smoke",
                "profile": "debug",
            },
            {
                "path": "packages/test/tests/smoke.ql",
                "kind": "smoke",
                "profile": "debug",
            },
            {
                "path": "examples/starter/tests/smoke.ql",
                "kind": "smoke",
                "profile": "debug",
            },
        ],
        "passed": 6,
        "failed": 0,
        "failures": [],
    });
    assert_eq!(
        actual, expected,
        "copied repo stdlib workspace should keep a stable full-workspace test json contract"
    );
}
