mod support;

use ql_driver::{ToolchainOptions, discover_toolchain};
use support::{
    TempDir, executable_output_path, expect_empty_stdout, expect_exit_code, expect_file_exists,
    expect_silent_output, expect_stderr_contains, ql_command, run_command_capture,
    static_library_output_path, workspace_root,
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
fn run_single_file_builds_and_executes_program() {
    if !toolchain_available("`ql run` single-file test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-file");
    let source_path = temp.write("demo.ql", "fn main() -> Int { return 7 }\n");
    let output_path = executable_output_path(&temp.path().join("target/ql/debug"), "demo");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["run"]).arg(&source_path);
    let output = run_command_capture(&mut command, "`ql run` single file");
    let (stdout, stderr) = expect_exit_code("project-run-file", "single-file run", &output, 7)
        .expect("single-file `ql run` should exit with the program status");
    expect_silent_output("project-run-file", "single-file run", &stdout, &stderr)
        .expect("single-file `ql run` should leave stdout/stderr to the program");
    expect_file_exists(
        "project-run-file",
        &output_path,
        "single-file executable",
        "single-file run",
    )
    .expect("single-file `ql run` should leave the built executable in the default path");
}

#[test]
fn run_package_path_executes_the_only_runnable_target_with_program_args() {
    if !toolchain_available("`ql run` package test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-package");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create package source tree for run test");
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write("app/src/lib.ql", "pub fn helper() -> Int { return 1 }\n");
    temp.write("app/src/main.ql", "fn main() -> Int { return 9 }\n");
    let output_path = executable_output_path(&project_root.join("target/ql/debug"), "main");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["run"])
        .arg(&project_root)
        .arg("--")
        .args(["alpha", "beta"]);
    let output = run_command_capture(&mut command, "`ql run` package path");
    let (stdout, stderr) = expect_exit_code("project-run-package", "package path run", &output, 9)
        .expect("package-path `ql run` should exit with the runnable target status");
    expect_silent_output("project-run-package", "package path run", &stdout, &stderr)
        .expect("package-path `ql run` should leave stdout/stderr to the program");
    expect_file_exists(
        "project-run-package",
        &output_path,
        "package executable",
        "package path run",
    )
    .expect("package-path `ql run` should leave the built executable in the package target dir");
}

#[test]
fn run_project_source_file_uses_project_aware_target_and_profile() {
    if !toolchain_available("`ql run` direct project source file test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-source-file-project-aware");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src/bin"))
        .expect("create package source tree for direct project source run test");
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"

[profile]
default = "release"
"#,
    );
    let main_path = temp.write("app/src/main.ql", "fn main() -> Int { return 13 }\n");
    temp.write("app/src/bin/admin.ql", "fn main() -> Int { return 2 }\n");
    let output_path = executable_output_path(&project_root.join("target/ql/release"), "main");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["run"]).arg(&main_path);
    let output = run_command_capture(&mut command, "`ql run` direct project source file");
    let (stdout, stderr) = expect_exit_code(
        "project-run-source-file-project-aware",
        "direct project source file run",
        &output,
        13,
    )
    .expect("direct project source file `ql run` should execute the selected target");
    expect_silent_output(
        "project-run-source-file-project-aware",
        "direct project source file run",
        &stdout,
        &stderr,
    )
    .expect("direct project source file `ql run` should leave stdout/stderr to the program");
    expect_file_exists(
        "project-run-source-file-project-aware",
        &output_path,
        "direct project source executable",
        "direct project source file run",
    )
    .expect("direct project source file `ql run` should emit the executable under the package target dir");
}

#[test]
fn run_package_path_uses_manifest_default_release_profile() {
    if !toolchain_available("`ql run` manifest profile test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-manifest-profile");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create package source tree for run manifest profile test");
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"

[profile]
default = "release"
"#,
    );
    temp.write("app/src/main.ql", "fn main() -> Int { return 13 }\n");
    let output_path = executable_output_path(&project_root.join("target/ql/release"), "main");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["run"]).arg(&project_root);
    let output = run_command_capture(&mut command, "`ql run` manifest default profile");
    let (stdout, stderr) = expect_exit_code(
        "project-run-manifest-profile",
        "manifest default profile run",
        &output,
        13,
    )
    .expect("package-path `ql run` should honor the manifest default profile");
    expect_silent_output(
        "project-run-manifest-profile",
        "manifest default profile run",
        &stdout,
        &stderr,
    )
    .expect("manifest default profile run should leave stdout/stderr to the program");
    expect_file_exists(
        "project-run-manifest-profile",
        &output_path,
        "manifest default profile executable",
        "manifest default profile run",
    )
    .expect("manifest default profile run should emit the release executable");
}

#[test]
fn run_workspace_path_uses_workspace_default_profile() {
    if !toolchain_available("`ql run` workspace profile test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-workspace-profile");
    let project_root = temp.path().join("workspace");
    std::fs::create_dir_all(project_root.join("packages/app/src"))
        .expect("create workspace package source tree for workspace profile run test");
    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app"]

[profile]
default = "release"
"#,
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace/packages/app/src/main.ql",
        "fn main() -> Int { return 13 }\n",
    );
    let output_path =
        executable_output_path(&project_root.join("packages/app/target/ql/release"), "main");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["run"]).arg(&project_root);
    let output = run_command_capture(&mut command, "`ql run` workspace default profile");
    let (stdout, stderr) = expect_exit_code(
        "project-run-workspace-profile",
        "workspace default profile run",
        &output,
        13,
    )
    .expect("workspace-path `ql run` should honor the workspace default profile");
    expect_silent_output(
        "project-run-workspace-profile",
        "workspace default profile run",
        &stdout,
        &stderr,
    )
    .expect("workspace default profile run should leave stdout/stderr to the program");
    expect_file_exists(
        "project-run-workspace-profile",
        &output_path,
        "workspace default profile executable",
        "workspace default profile run",
    )
    .expect("workspace default profile run should emit the release executable");
}

#[test]
fn run_workspace_member_source_file_uses_workspace_default_profile() {
    if !toolchain_available("`ql run` workspace source profile test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-workspace-source-profile");
    let project_root = temp.path().join("workspace");
    std::fs::create_dir_all(project_root.join("packages/app/src"))
        .expect("create workspace package source tree for workspace source profile run test");
    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app"]

[profile]
default = "release"
"#,
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    let main_path = temp.write(
        "workspace/packages/app/src/main.ql",
        "fn main() -> Int { return 17 }\n",
    );
    let output_path =
        executable_output_path(&project_root.join("packages/app/target/ql/release"), "main");
    let debug_output_path =
        executable_output_path(&project_root.join("packages/app/target/ql/debug"), "main");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["run"]).arg(&main_path);
    let output = run_command_capture(
        &mut command,
        "`ql run` workspace member source default profile",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-run-workspace-source-profile",
        "workspace member source default profile run",
        &output,
        17,
    )
    .expect("workspace member source path `ql run` should honor the workspace default profile");
    expect_silent_output(
        "project-run-workspace-source-profile",
        "workspace member source default profile run",
        &stdout,
        &stderr,
    )
    .expect(
        "workspace member source default profile run should leave stdout/stderr to the program",
    );
    expect_file_exists(
        "project-run-workspace-source-profile",
        &output_path,
        "workspace member source default profile executable",
        "workspace member source default profile run",
    )
    .expect("workspace member source default profile run should emit the release executable");
    assert!(
        !debug_output_path.exists(),
        "workspace member source default profile run should not silently fall back to the debug profile"
    );
}

#[test]
fn run_workspace_path_executes_the_only_runnable_target() {
    if !toolchain_available("`ql run` workspace test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-workspace");
    let project_root = temp.path().join("workspace");
    std::fs::create_dir_all(project_root.join("packages/app/src"))
        .expect("create app package source tree");
    std::fs::create_dir_all(project_root.join("packages/tool/src"))
        .expect("create tool package source tree");
    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/tool"]
"#,
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace/packages/tool/qlang.toml",
        r#"
[package]
name = "tool"
"#,
    );
    temp.write(
        "workspace/packages/app/src/main.ql",
        "fn main() -> Int { return 11 }\n",
    );
    temp.write(
        "workspace/packages/tool/src/lib.ql",
        "pub fn helper() -> Int { return 2 }\n",
    );
    let output_path =
        executable_output_path(&project_root.join("packages/app/target/ql/debug"), "main");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["run"]).arg(&project_root);
    let output = run_command_capture(&mut command, "`ql run` workspace path");
    let (stdout, stderr) =
        expect_exit_code("project-run-workspace", "workspace path run", &output, 11)
            .expect("workspace-path `ql run` should exit with the runnable member status");
    expect_silent_output(
        "project-run-workspace",
        "workspace path run",
        &stdout,
        &stderr,
    )
    .expect("workspace-path `ql run` should leave stdout/stderr to the program");
    expect_file_exists(
        "project-run-workspace",
        &output_path,
        "workspace executable",
        "workspace path run",
    )
    .expect("workspace-path `ql run` should leave the built executable in the member target dir");
}

#[test]
fn run_preserves_large_exit_code() {
    if !toolchain_available("`ql run` large-exit-code test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-large-exit");
    let source_path = temp.write("large_exit.ql", "fn main() -> Int { return 690 }\n");
    let output_path = executable_output_path(&temp.path().join("target/ql/debug"), "large_exit");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["run"]).arg(&source_path);
    let output = run_command_capture(&mut command, "`ql run` large exit code");
    let (stdout, stderr) = expect_exit_code(
        "project-run-large-exit",
        "large-exit-code run",
        &output,
        690,
    )
    .expect("`ql run` should preserve the child exit code");
    expect_silent_output(
        "project-run-large-exit",
        "large-exit-code run",
        &stdout,
        &stderr,
    )
    .expect("large-exit-code `ql run` should leave stdout/stderr to the program");
    expect_file_exists(
        "project-run-large-exit",
        &output_path,
        "large-exit executable",
        "large-exit-code run",
    )
    .expect("large-exit-code `ql run` should still leave the built executable in place");
}

#[test]
fn run_project_path_rejects_multiple_runnable_targets() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-multiple");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src/bin"))
        .expect("create package source tree for multi-target run test");
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write("app/src/main.ql", "fn main() -> Int { return 1 }\n");
    temp.write("app/src/bin/admin.ql", "fn main() -> Int { return 2 }\n");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["run"]).arg(&project_root);
    let output = run_command_capture(&mut command, "`ql run` multiple runnable targets");
    let (stdout, stderr) = expect_exit_code(
        "project-run-multiple",
        "multiple runnable target rejection",
        &output,
        1,
    )
    .expect("`ql run` should reject project paths with multiple runnable targets");
    expect_empty_stdout(
        "project-run-multiple",
        "multiple runnable target rejection",
        &stdout,
    )
    .expect("multiple runnable target rejection should not print stdout");
    expect_stderr_contains(
        "project-run-multiple",
        "multiple runnable target rejection",
        &stderr,
        "error: `ql run` found multiple runnable build targets",
    )
    .expect("multiple runnable target rejection should explain the ambiguity");
    expect_stderr_contains(
        "project-run-multiple",
        "multiple runnable target rejection",
        &stderr,
        "hint: rerun `ql run <source-file>`",
    )
    .expect("multiple runnable target rejection should point to a direct target rerun");
}

#[test]
fn run_project_path_selects_requested_binary_target() {
    if !toolchain_available("`ql run --bin` package test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-select-bin");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src/bin"))
        .expect("create package source tree for target selector run test");
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write("app/src/main.ql", "fn main() -> Int { return 1 }\n");
    temp.write("app/src/bin/admin.ql", "fn main() -> Int { return 2 }\n");
    let output_path = executable_output_path(&project_root.join("target/ql/debug/bin"), "admin");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command
        .args(["run"])
        .arg(&project_root)
        .args(["--bin", "admin"]);
    let output = run_command_capture(&mut command, "`ql run --bin` package path");
    let (stdout, stderr) = expect_exit_code(
        "project-run-select-bin",
        "selected binary target run",
        &output,
        2,
    )
    .expect("package-path `ql run --bin` should exit with the selected binary status");
    expect_silent_output(
        "project-run-select-bin",
        "selected binary target run",
        &stdout,
        &stderr,
    )
    .expect("package-path `ql run --bin` should leave stdout/stderr to the program");
    expect_file_exists(
        "project-run-select-bin",
        &output_path,
        "selected binary executable",
        "selected binary target run",
    )
    .expect(
        "package-path `ql run --bin` should build the selected executable in the bin target dir",
    );
}

#[test]
fn run_library_only_package_reports_no_runnable_targets() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-library-only");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create package source tree for no-runnable-target test");
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write("app/src/lib.ql", "pub fn helper() -> Int { return 1 }\n");

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["run"]).arg(&project_root);
    let output = run_command_capture(&mut command, "`ql run` library-only package");
    let (stdout, stderr) = expect_exit_code(
        "project-run-library-only",
        "library-only run rejection",
        &output,
        1,
    )
    .expect("`ql run` should reject packages without runnable targets");
    expect_empty_stdout(
        "project-run-library-only",
        "library-only run rejection",
        &stdout,
    )
    .expect("library-only run rejection should not print stdout");
    expect_stderr_contains(
        "project-run-library-only",
        "library-only run rejection",
        &stderr,
        "error: `ql run` found no runnable build targets",
    )
    .expect("library-only run rejection should explain the missing runnable target");
    expect_stderr_contains(
        "project-run-library-only",
        "library-only run rejection",
        &stderr,
        "hint: add `src/main.ql`, `src/bin/*.ql`, or declare `[[bin]].path`",
    )
    .expect("library-only run rejection should explain how to make the package runnable");
}

#[test]
fn run_package_path_syncs_dependency_interfaces_without_polluting_program_output() {
    if !toolchain_available("`ql run` dependency sync test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-run-dependency-sync");
    let dep_root = temp.path().join("dep");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(dep_root.join("src")).expect("create dependency source tree");
    std::fs::create_dir_all(project_root.join("src")).expect("create package source tree");
    temp.write(
        "dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "dep/src/lib.ql",
        "extern \"c\" pub fn q_add(left: Int, right: Int) -> Int { return left + right }\n",
    );
    temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"

[dependencies]
dep = "../dep"
"#,
    );
    temp.write(
        "app/src/main.ql",
        "use dep.q_add as add\n\nfn main() -> Int { return add(6, 7) }\n",
    );

    let interface_output = dep_root.join("dep.qi");
    let dependency_output = static_library_output_path(&dep_root.join("target/ql/debug"), "lib");
    let executable_output = executable_output_path(&project_root.join("target/ql/debug"), "main");
    assert!(
        !interface_output.exists(),
        "dependency interface should start missing for sync test"
    );

    let mut command = ql_command(&workspace_root);
    command.current_dir(temp.path());
    command.args(["run"]).arg(&project_root);
    let output = run_command_capture(&mut command, "`ql run` dependency sync");
    let (stdout, stderr) = expect_exit_code(
        "project-run-dependency-sync",
        "package path run with dependency sync",
        &output,
        13,
    )
    .expect("package-path `ql run` should sync dependency interfaces before execution");
    expect_silent_output(
        "project-run-dependency-sync",
        "package path run with dependency sync",
        &stdout,
        &stderr,
    )
    .expect("dependency-sync run should leave stdout/stderr to the program");
    expect_file_exists(
        "project-run-dependency-sync",
        &interface_output,
        "synced dependency interface",
        "package path run with dependency sync",
    )
    .expect("dependency-sync run should emit the dependency interface");
    expect_file_exists(
        "project-run-dependency-sync",
        &dependency_output,
        "dependency package artifact",
        "package path run with dependency sync",
    )
    .expect("dependency-sync run should also build the dependency package artifact");
    expect_file_exists(
        "project-run-dependency-sync",
        &executable_output,
        "package executable",
        "package path run with dependency sync",
    )
    .expect("dependency-sync run should still emit the executable artifact");
}
