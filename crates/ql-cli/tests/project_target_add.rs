mod support;

use std::process::Stdio;

use support::{
    TempDir, assert_no_atomic_write_temp_files, assert_no_build_lock_directories,
    expect_empty_stderr, expect_stdout_contains_all, expect_success, ql_command,
    read_normalized_file, run_command_capture, workspace_root,
};

#[test]
fn project_target_add_bin_preserves_existing_conventional_targets() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-target-add-bin");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src/bin")).expect("create package bin source tree");

    let manifest_path = temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write("app/src/lib.ql", "pub fn util() -> Int { return 1 }\n");
    temp.write("app/src/main.ql", "fn main() -> Int { return 0 }\n");
    temp.write("app/src/bin/admin.ql", "fn main() -> Int { return 1 }\n");

    let mut add = ql_command(&workspace_root);
    add.args(["project", "target", "add", "--bin", "worker"])
        .arg(&project_root);
    let output = run_command_capture(&mut add, "`ql project target add --bin` package");
    let (stdout, stderr) = expect_success(
        "project-target-add-bin",
        "package binary target add",
        &output,
    )
    .expect("package binary target add should succeed");
    expect_empty_stderr(
        "project-target-add-bin",
        "package binary target add",
        &stderr,
    )
    .expect("package binary target add should not print stderr");
    expect_stdout_contains_all(
        "project-target-add-bin",
        &stdout.replace('\\', "/"),
        &[
            &format!(
                "updated: {}",
                manifest_path.to_string_lossy().replace('\\', "/")
            ),
            &format!(
                "created: {}",
                project_root
                    .join("src/bin/worker.ql")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
        ],
    )
    .expect("package binary target add should report the updated manifest and created source");

    let manifest =
        read_normalized_file(&project_root.join("qlang.toml"), "updated package manifest");
    assert!(
        manifest.contains("[[bin]]\npath = \"src/main.ql\"\n")
            && manifest.contains("[[bin]]\npath = \"src/bin/admin.ql\"\n")
            && manifest.contains("[[bin]]\npath = \"src/bin/worker.ql\"\n"),
        "binary target add should preserve existing conventional binaries and append the new one, got:\n{manifest}"
    );
    assert_eq!(
        read_normalized_file(
            &project_root.join("src/bin/worker.ql"),
            "added binary target source"
        ),
        "fn main() -> Int {\n    return 0\n}\n"
    );

    let mut targets = ql_command(&workspace_root);
    targets.args(["project", "targets"]).arg(&project_root);
    let output = run_command_capture(&mut targets, "`ql project targets` after target add");
    let (stdout, stderr) = expect_success(
        "project-target-add-bin",
        "package target discovery after binary add",
        &output,
    )
    .expect("package target discovery after binary add should succeed");
    expect_empty_stderr(
        "project-target-add-bin",
        "package target discovery after binary add",
        &stderr,
    )
    .expect("package target discovery after binary add should not print stderr");
    expect_stdout_contains_all(
        "project-target-add-bin",
        &stdout.replace('\\', "/"),
        &[
            &format!(
                "manifest: {}",
                manifest_path.to_string_lossy().replace('\\', "/")
            ),
            "  - bin: src/main.ql",
            "  - bin: src/bin/admin.ql",
            "  - bin: src/bin/worker.ql",
        ],
    )
    .expect("package target discovery after binary add should include preserved and newly added binaries");
}

#[test]
fn project_target_add_serializes_concurrent_package_manifest_writes() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-target-add-concurrent-writes");
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src")).expect("create package source tree");

    let manifest_path = temp.write(
        "app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write("app/src/lib.ql", "pub fn util() -> Int { return 1 }\n");
    temp.write("app/src/main.ql", "fn main() -> Int { return 0 }\n");

    let mut children = Vec::new();
    for binary_name in ["worker", "admin"] {
        let mut add = ql_command(&workspace_root);
        add.current_dir(temp.path());
        add.args(["project", "target", "add", "--bin", binary_name])
            .arg(&project_root);
        add.stdout(Stdio::piped()).stderr(Stdio::piped());
        let child = add
            .spawn()
            .unwrap_or_else(|error| panic!("spawn concurrent target add `{binary_name}`: {error}"));
        children.push((binary_name, child));
    }

    for (binary_name, child) in children {
        let output = child.wait_with_output().unwrap_or_else(|error| {
            panic!("wait for concurrent target add `{binary_name}`: {error}")
        });
        let (stdout, stderr) = expect_success(
            "project-target-add-concurrent-writes",
            &format!("concurrent target add `{binary_name}`"),
            &output,
        )
        .unwrap_or_else(|message| panic!("{message}"));
        expect_empty_stderr(
            "project-target-add-concurrent-writes",
            &format!("concurrent target add `{binary_name}`"),
            &stderr,
        )
        .unwrap_or_else(|message| panic!("{message}"));
        expect_stdout_contains_all(
            "project-target-add-concurrent-writes",
            &stdout.replace('\\', "/"),
            &[
                &format!(
                    "updated: {}",
                    manifest_path.to_string_lossy().replace('\\', "/")
                ),
                &format!(
                    "created: {}",
                    project_root
                        .join(format!("src/bin/{binary_name}.ql"))
                        .to_string_lossy()
                        .replace('\\', "/")
                ),
            ],
        )
        .unwrap_or_else(|message| panic!("{message}"));
    }

    let manifest = read_normalized_file(
        &manifest_path,
        "package manifest after concurrent target adds",
    );
    assert!(
        manifest.contains("[[bin]]\npath = \"src/main.ql\"\n")
            && manifest.contains("[[bin]]\npath = \"src/bin/admin.ql\"\n")
            && manifest.contains("[[bin]]\npath = \"src/bin/worker.ql\"\n"),
        "concurrent target add should keep both new targets and the conventional main target, got:\n{manifest}"
    );
    assert!(
        project_root.join("src/bin/admin.ql").is_file()
            && project_root.join("src/bin/worker.ql").is_file(),
        "concurrent target add should create both binary source files"
    );
    assert_no_build_lock_directories("project-target-add-concurrent-writes", &project_root);
    assert_no_atomic_write_temp_files("project-target-add-concurrent-writes", &project_root);
}

#[test]
fn project_target_add_bin_supports_workspace_root_package_selector() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-target-add-workspace-selector");
    let project_root = temp.path().join("workspace");
    std::fs::create_dir_all(project_root.join("packages/app/src"))
        .expect("create workspace member source tree");

    let manifest_path = temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app"]
"#,
    );
    temp.write(
        "workspace/packages/app/src/lib.ql",
        "pub fn util() -> Int { return 1 }\n",
    );
    temp.write(
        "workspace/packages/app/src/main.ql",
        "fn main() -> Int { return 0 }\n",
    );

    let mut add = ql_command(&workspace_root);
    add.args([
        "project",
        "target",
        "add",
        &project_root.to_string_lossy(),
        "--package",
        "app",
        "--bin",
        "worker",
    ]);
    let output = run_command_capture(
        &mut add,
        "`ql project target add --package --bin` workspace root",
    );
    let (stdout, stderr) = expect_success(
        "project-target-add-workspace-selector",
        "workspace root binary target add",
        &output,
    )
    .expect("workspace root binary target add should succeed");
    expect_empty_stderr(
        "project-target-add-workspace-selector",
        "workspace root binary target add",
        &stderr,
    )
    .expect("workspace root binary target add should not print stderr");
    expect_stdout_contains_all(
        "project-target-add-workspace-selector",
        &stdout.replace('\\', "/"),
        &[
            &format!(
                "updated: {}",
                manifest_path.to_string_lossy().replace('\\', "/")
            ),
            &format!(
                "created: {}",
                project_root
                    .join("packages/app/src/bin/worker.ql")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
        ],
    )
    .expect(
        "workspace root binary target add should report the updated manifest and created source",
    );

    let manifest = read_normalized_file(
        &project_root.join("packages/app/qlang.toml"),
        "workspace member manifest after binary target add",
    );
    assert!(
        manifest.contains("[[bin]]\npath = \"src/main.ql\"\n")
            && manifest.contains("[[bin]]\npath = \"src/bin/worker.ql\"\n"),
        "workspace root binary target add should preserve the default main target and append the new one, got:\n{manifest}"
    );
    assert_eq!(
        read_normalized_file(
            &project_root.join("packages/app/src/bin/worker.ql"),
            "workspace member binary target source",
        ),
        "fn main() -> Int {\n    return 0\n}\n"
    );
}

#[test]
fn project_target_add_bin_supports_workspace_member_directory() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-target-add-workspace-member-dir");
    let project_root = temp.path().join("workspace");
    let member_dir = project_root.join("packages/app");
    std::fs::create_dir_all(member_dir.join("src")).expect("create workspace member source tree");

    let manifest_path = temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app"]
"#,
    );
    temp.write(
        "workspace/packages/app/src/lib.ql",
        "pub fn util() -> Int { return 1 }\n",
    );
    temp.write(
        "workspace/packages/app/src/main.ql",
        "fn main() -> Int { return 0 }\n",
    );

    let mut add = ql_command(&workspace_root);
    add.args([
        "project",
        "target",
        "add",
        &member_dir.to_string_lossy(),
        "--bin",
        "worker",
    ]);
    let output = run_command_capture(
        &mut add,
        "`ql project target add --bin` workspace member directory",
    );
    let (stdout, stderr) = expect_success(
        "project-target-add-workspace-member-dir",
        "workspace member directory binary target add",
        &output,
    )
    .expect("workspace member directory binary target add should succeed");
    expect_empty_stderr(
        "project-target-add-workspace-member-dir",
        "workspace member directory binary target add",
        &stderr,
    )
    .expect("workspace member directory binary target add should not print stderr");
    expect_stdout_contains_all(
        "project-target-add-workspace-member-dir",
        &stdout.replace('\\', "/"),
        &[
            &format!(
                "updated: {}",
                manifest_path.to_string_lossy().replace('\\', "/")
            ),
            &format!(
                "created: {}",
                member_dir
                    .join("src/bin/worker.ql")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
        ],
    )
    .expect(
        "workspace member directory binary target add should report the updated manifest and created source",
    );

    let manifest = read_normalized_file(
        &project_root.join("packages/app/qlang.toml"),
        "workspace member manifest after member directory binary target add",
    );
    assert!(
        manifest.contains("[[bin]]\npath = \"src/main.ql\"\n")
            && manifest.contains("[[bin]]\npath = \"src/bin/worker.ql\"\n"),
        "workspace member directory binary target add should preserve the default main target and append the new one, got:\n{manifest}"
    );
    assert_eq!(
        read_normalized_file(
            &member_dir.join("src/bin/worker.ql"),
            "workspace member directory binary target source",
        ),
        "fn main() -> Int {\n    return 0\n}\n"
    );
}
