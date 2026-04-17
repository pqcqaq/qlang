mod support;

use support::{
    TempDir, expect_empty_stderr, expect_empty_stdout, expect_exit_code, expect_snapshot_matches,
    expect_stderr_contains, expect_stdout_contains_all, expect_success, ql_command,
    read_normalized_file, run_command_capture, workspace_root,
};

#[test]
fn project_lock_writes_workspace_lockfile_with_external_dependencies() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-lock-workspace");
    let project_root = temp.path().join("workspace");
    std::fs::create_dir_all(project_root.join("packages/app/src/tools"))
        .expect("create app source tree for workspace lock test");
    std::fs::create_dir_all(project_root.join("packages/core/src/runtime"))
        .expect("create core source tree for workspace lock test");
    std::fs::create_dir_all(project_root.join("vendor/runtime/src"))
        .expect("create runtime source tree for workspace lock test");

    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/core"]

[profile]
default = "release"
"#,
    );
    temp.write(
        "workspace/packages/core/qlang.toml",
        r#"
[package]
name = "core"

[lib]
path = "src/runtime/core.ql"
"#,
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"

[dependencies]
core = { path = "../core" }
runtime = { path = "../../vendor/runtime" }

[[bin]]
path = "src/tools/app.ql"
"#,
    );
    temp.write(
        "workspace/vendor/runtime/qlang.toml",
        r#"
[package]
name = "runtime"

[profile]
default = "debug"
"#,
    );
    temp.write(
        "workspace/packages/core/src/runtime/core.ql",
        "pub fn core() -> Int { return 1 }\n",
    );
    temp.write(
        "workspace/packages/app/src/tools/app.ql",
        "fn main() -> Int { return 0 }\n",
    );
    temp.write(
        "workspace/vendor/runtime/src/lib.ql",
        "pub fn runtime() -> Int { return 2 }\n",
    );

    let lockfile_path = project_root.join("qlang.lock");
    let lockfile_display = lockfile_path.to_string_lossy().replace('\\', "/");

    let mut command = ql_command(&workspace_root);
    command.args(["project", "lock"]).arg(&project_root);
    let output = run_command_capture(&mut command, "`ql project lock` workspace");
    let (stdout, stderr) = expect_success(
        "project-lock-workspace",
        "workspace lockfile generation",
        &output,
    )
    .expect("workspace lockfile generation should succeed");
    expect_empty_stderr(
        "project-lock-workspace",
        "workspace lockfile generation",
        &stderr,
    )
    .expect("workspace lockfile generation should not print stderr");
    expect_stdout_contains_all(
        "project-lock-workspace",
        &stdout.replace('\\', "/"),
        &[&format!("wrote lockfile: {lockfile_display}")],
    )
    .expect("workspace lockfile generation should report the written lockfile path");

    let expected = r#"{
  "packages": [
    {
      "default_profile": "release",
      "dependencies": [],
      "manifest_path": "packages/core/qlang.toml",
      "package_name": "core",
      "selected": true,
      "targets": [
        {
          "kind": "lib",
          "path": "packages/core/src/runtime/core.ql"
        }
      ]
    },
    {
      "default_profile": "debug",
      "dependencies": [],
      "manifest_path": "vendor/runtime/qlang.toml",
      "package_name": "runtime",
      "selected": false,
      "targets": [
        {
          "kind": "lib",
          "path": "vendor/runtime/src/lib.ql"
        }
      ]
    },
    {
      "default_profile": "release",
      "dependencies": [
        "packages/core/qlang.toml",
        "vendor/runtime/qlang.toml"
      ],
      "manifest_path": "packages/app/qlang.toml",
      "package_name": "app",
      "selected": true,
      "targets": [
        {
          "kind": "bin",
          "path": "packages/app/src/tools/app.ql"
        }
      ]
    }
  ],
  "root": {
    "kind": "workspace",
    "manifest_path": "qlang.toml"
  },
  "schema": "ql.project.lock.v1",
  "workspace_members": [
    "packages/core/qlang.toml",
    "packages/app/qlang.toml"
  ]
}
"#;
    let actual = read_normalized_file(&lockfile_path, "workspace lockfile");
    expect_snapshot_matches(
        "project-lock-workspace",
        "workspace lockfile contents",
        expected,
        &actual,
    )
    .expect("workspace lockfile should match the stable schema contract");
}

#[test]
fn project_lock_check_fails_when_lockfile_is_stale() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-lock-stale");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create package source tree for stale lock test");

    let manifest_path = temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"

[profile]
default = "release"
"#,
    );
    temp.write("app/src/main.ql", "fn main() -> Int { return 0 }\n");

    let lockfile_path = project_root.join("qlang.lock");
    let mut write_command = ql_command(&workspace_root);
    write_command.args(["project", "lock"]).arg(&project_root);
    let write_output = run_command_capture(&mut write_command, "`ql project lock` package");
    let (_, write_stderr) = expect_success(
        "project-lock-stale",
        "initial lockfile generation",
        &write_output,
    )
    .expect("initial lockfile generation should succeed");
    expect_empty_stderr(
        "project-lock-stale",
        "initial lockfile generation",
        &write_stderr,
    )
    .expect("initial lockfile generation should not print stderr");
    let initial_lockfile = read_normalized_file(&lockfile_path, "initial lockfile");

    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"

[profile]
default = "debug"
"#,
    );

    let mut check_command = ql_command(&workspace_root);
    check_command
        .args(["project", "lock", "--check"])
        .arg(&project_root);
    let output = run_command_capture(&mut check_command, "`ql project lock --check` stale");
    let (stdout, stderr) =
        expect_exit_code("project-lock-stale", "stale lockfile check", &output, 1)
            .expect("stale lockfile check should fail with exit code 1");
    expect_empty_stdout("project-lock-stale", "stale lockfile check", &stdout)
        .expect("stale lockfile check should not print stdout");

    let normalized_stderr = stderr.replace('\\', "/");
    let lockfile_display = lockfile_path.to_string_lossy().replace('\\', "/");
    let manifest_display = manifest_path.to_string_lossy().replace('\\', "/");
    expect_stderr_contains(
        "project-lock-stale",
        "stale lockfile check",
        &normalized_stderr,
        &format!("error: `ql project lock --check` lockfile `{lockfile_display}` is stale"),
    )
    .expect("stale lockfile check should report a stale lockfile");
    expect_stderr_contains(
        "project-lock-stale",
        "stale lockfile check",
        &normalized_stderr,
        &format!("note: failing package manifest: {manifest_display}"),
    )
    .expect("stale lockfile check should point to the package manifest");
    expect_stderr_contains(
        "project-lock-stale",
        "stale lockfile check",
        &normalized_stderr,
        &format!("hint: rerun `ql project lock {manifest_display}` to regenerate `qlang.lock`"),
    )
    .expect("stale lockfile check should preserve the direct regeneration hint");

    let unchanged_lockfile = read_normalized_file(&lockfile_path, "stale lockfile");
    expect_snapshot_matches(
        "project-lock-stale",
        "stale lockfile contents",
        &initial_lockfile,
        &unchanged_lockfile,
    )
    .expect("stale lockfile check should not rewrite the existing lockfile");
}
