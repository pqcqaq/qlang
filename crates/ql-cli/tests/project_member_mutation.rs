mod support;

use std::process::Stdio;

use support::{
    TempDir, assert_no_atomic_write_temp_files, assert_no_build_lock_directories,
    expect_empty_stderr, expect_stdout_contains_all, expect_success, ql_command,
    read_normalized_file, run_command_capture, workspace_root,
};

#[test]
fn project_add_creates_workspace_member_from_member_source_path() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-add-success");
    let project_root = temp.path().join("workspace");
    let request_path = project_root.join("packages/app/src/main.ql");

    let mut init = ql_command(&workspace_root);
    init.args([
        "project",
        "init",
        &project_root.to_string_lossy(),
        "--workspace",
        "--name",
        "app",
    ]);
    let output = run_command_capture(&mut init, "`ql project init` workspace for add");
    let (_stdout, stderr) =
        expect_success("project-add-success", "workspace init for add", &output).unwrap();
    expect_empty_stderr("project-add-success", "workspace init for add", &stderr).unwrap();

    let mut add_core = ql_command(&workspace_root);
    add_core.args([
        "project",
        "add",
        &project_root.to_string_lossy(),
        "--name",
        "core",
    ]);
    let output = run_command_capture(&mut add_core, "`ql project add` workspace core member");
    let (_stdout, stderr) =
        expect_success("project-add-success", "add workspace core member", &output).unwrap();
    expect_empty_stderr("project-add-success", "add workspace core member", &stderr).unwrap();

    let mut add = ql_command(&workspace_root);
    add.args([
        "project",
        "add",
        &request_path.to_string_lossy(),
        "--name",
        "tools",
        "--dependency",
        "app",
        "--dependency",
        "core",
    ]);
    let output = run_command_capture(&mut add, "`ql project add` workspace member source path");
    let (stdout, stderr) =
        expect_success("project-add-success", "add workspace member", &output).unwrap();
    expect_empty_stderr("project-add-success", "add workspace member", &stderr).unwrap();
    expect_stdout_contains_all(
        "project-add-success",
        &stdout,
        &[
            &format!(
                "updated: {}",
                project_root
                    .join("qlang.toml")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            &format!(
                "created: {}",
                project_root
                    .join("packages/tools/qlang.toml")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            &format!(
                "created: {}",
                project_root
                    .join("packages/tools/src/lib.ql")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            &format!(
                "created: {}",
                project_root
                    .join("packages/tools/src/main.ql")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            &format!(
                "created: {}",
                project_root
                    .join("packages/tools/tests/smoke.ql")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
        ],
    )
    .unwrap();

    let workspace_manifest = read_normalized_file(
        &project_root.join("qlang.toml"),
        "workspace manifest after add",
    );
    assert!(
        workspace_manifest.contains("packages/app"),
        "workspace manifest should keep existing member entry"
    );
    assert!(
        workspace_manifest.contains("packages/tools"),
        "workspace manifest should add the new member entry"
    );
    assert_eq!(
        read_normalized_file(
            &project_root.join("packages/tools/qlang.toml"),
            "added workspace member manifest"
        ),
        "[package]\nname = \"tools\"\n\n[dependencies]\napp = \"../app\"\ncore = \"../core\"\n"
    );
    assert_eq!(
        read_normalized_file(
            &project_root.join("packages/tools/src/lib.ql"),
            "added workspace member lib"
        ),
        "pub fn run() -> Int {\n    return 0\n}\n"
    );
    assert_eq!(
        read_normalized_file(
            &project_root.join("packages/tools/tests/smoke.ql"),
            "added workspace member smoke test"
        ),
        "fn main() -> Int {\n    return 0\n}\n"
    );

    let mut graph = ql_command(&workspace_root);
    graph.args(["project", "graph", &project_root.to_string_lossy()]);
    let output = run_command_capture(&mut graph, "`ql project graph` after add");
    let (stdout, stderr) =
        expect_success("project-add-success", "graph workspace after add", &output).unwrap();
    expect_empty_stderr("project-add-success", "graph workspace after add", &stderr).unwrap();
    expect_stdout_contains_all(
        "project-add-success",
        &stdout,
        &[
            "workspace_members:",
            "  - packages/app",
            "  - packages/core",
            "  - packages/tools",
            "  - member: packages/core",
            "    package: core",
            "  - member: packages/tools",
            "    package: tools",
            "    status: missing",
            "    references:",
            "      - ../app",
            "      - ../core",
        ],
    )
    .unwrap();
}

#[test]
fn project_member_updates_serialize_concurrent_workspace_manifest_writes() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-member-concurrent-writes");
    let project_root = temp.path().join("workspace");
    let workspace_manifest_path = project_root.join("qlang.toml");

    temp.write(
        "workspace/qlang.toml",
        "[workspace]\nmembers = [\"packages/app\", \"packages/core\"]\n",
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        "[package]\nname = \"app\"\n",
    );
    temp.write(
        "workspace/packages/core/qlang.toml",
        "[package]\nname = \"core\"\n",
    );

    let mut children = Vec::new();

    let mut add_util = ql_command(&workspace_root);
    add_util.current_dir(temp.path());
    add_util.args([
        "project",
        "add",
        &project_root.to_string_lossy(),
        "--name",
        "util",
    ]);
    add_util.stdout(Stdio::piped()).stderr(Stdio::piped());
    let add_child = add_util
        .spawn()
        .unwrap_or_else(|error| panic!("spawn concurrent ql project add: {error}"));
    children.push(("add util", add_child));

    let mut remove_core = ql_command(&workspace_root);
    remove_core.current_dir(temp.path());
    remove_core.args([
        "project",
        "remove",
        &project_root.to_string_lossy(),
        "--name",
        "core",
    ]);
    remove_core.stdout(Stdio::piped()).stderr(Stdio::piped());
    let remove_child = remove_core
        .spawn()
        .unwrap_or_else(|error| panic!("spawn concurrent ql project remove: {error}"));
    children.push(("remove core", remove_child));

    for (action, child) in children {
        let output = child.wait_with_output().unwrap_or_else(|error| {
            panic!("wait for concurrent ql project member edit `{action}`: {error}")
        });
        let (stdout, stderr) = expect_success(
            "project-member-concurrent-writes",
            &format!("concurrent project member edit `{action}`"),
            &output,
        )
        .unwrap_or_else(|message| panic!("{message}"));
        expect_empty_stderr(
            "project-member-concurrent-writes",
            &format!("concurrent project member edit `{action}`"),
            &stderr,
        )
        .unwrap_or_else(|message| panic!("{message}"));
        expect_stdout_contains_all(
            "project-member-concurrent-writes",
            &stdout.replace('\\', "/"),
            &[&format!(
                "updated: {}",
                workspace_manifest_path.to_string_lossy().replace('\\', "/")
            )],
        )
        .unwrap_or_else(|message| panic!("{message}"));
    }

    let workspace_manifest = read_normalized_file(
        &workspace_manifest_path,
        "workspace manifest after concurrent member edits",
    );
    assert!(
        workspace_manifest.contains("packages/app"),
        "concurrent workspace edit should keep app: {workspace_manifest}"
    );
    assert!(
        workspace_manifest.contains("packages/util"),
        "concurrent workspace edit should add util: {workspace_manifest}"
    );
    assert!(
        !workspace_manifest.contains("packages/core"),
        "concurrent workspace edit should remove core: {workspace_manifest}"
    );
    assert!(
        project_root.join("packages/util/qlang.toml").is_file(),
        "concurrent add should create the util package scaffold"
    );
    assert_no_build_lock_directories("project-member-concurrent-writes", &project_root);
    assert_no_atomic_write_temp_files("project-member-concurrent-writes", &project_root);
}

#[test]
fn project_add_existing_workspace_member_from_source_path() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-add-existing");
    let project_root = temp.path().join("workspace");
    let existing_request_path = project_root.join("vendor/core/src/main.ql");

    let mut init = ql_command(&workspace_root);
    init.args([
        "project",
        "init",
        &project_root.to_string_lossy(),
        "--workspace",
        "--name",
        "app",
    ]);
    let output = run_command_capture(&mut init, "`ql project init` workspace for existing add");
    let (_stdout, stderr) = expect_success(
        "project-add-existing",
        "workspace init for existing add",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-add-existing",
        "workspace init for existing add",
        &stderr,
    )
    .unwrap();

    temp.write(
        "workspace/vendor/core/qlang.toml",
        "[package]\nname = \"core\"\n",
    );
    temp.write(
        "workspace/vendor/core/src/main.ql",
        "fn main() -> Int {\n    return 0\n}\n",
    );

    let mut add = ql_command(&workspace_root);
    add.args([
        "project",
        "add",
        &project_root.to_string_lossy(),
        "--existing",
        &existing_request_path.to_string_lossy(),
    ]);
    let output = run_command_capture(&mut add, "`ql project add --existing` source path");
    let (stdout, stderr) = expect_success(
        "project-add-existing",
        "add existing workspace member from source path",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-add-existing",
        "add existing workspace member from source path",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "project-add-existing",
        &stdout.replace('\\', "/"),
        &[
            &format!(
                "updated: {}",
                project_root
                    .join("qlang.toml")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            &format!(
                "added: {}",
                project_root
                    .join("vendor/core")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
        ],
    )
    .unwrap();

    assert_eq!(
        read_normalized_file(
            &project_root.join("qlang.toml"),
            "workspace manifest after existing member add"
        ),
        "[workspace]\nmembers = [\"packages/app\", \"vendor/core\"]\n"
    );
    assert_eq!(
        read_normalized_file(
            &project_root.join("vendor/core/qlang.toml"),
            "existing package manifest after workspace add"
        ),
        "[package]\nname = \"core\"\n"
    );
}

#[test]
fn project_remove_updates_workspace_members_from_member_source_path() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-remove-success");
    let project_root = temp.path().join("workspace");
    let request_path = project_root.join("packages/tools/src/main.ql");
    let removed_member_root = project_root.join("packages/tools");

    let mut init = ql_command(&workspace_root);
    init.args([
        "project",
        "init",
        &project_root.to_string_lossy(),
        "--workspace",
        "--name",
        "app",
    ]);
    let output = run_command_capture(&mut init, "`ql project init` workspace for remove");
    let (_stdout, stderr) = expect_success(
        "project-remove-success",
        "workspace init for remove",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-remove-success",
        "workspace init for remove",
        &stderr,
    )
    .unwrap();

    let mut add_core = ql_command(&workspace_root);
    add_core.args([
        "project",
        "add",
        &project_root.to_string_lossy(),
        "--name",
        "core",
    ]);
    let output = run_command_capture(&mut add_core, "`ql project add` core for remove");
    let (_stdout, stderr) = expect_success(
        "project-remove-success",
        "workspace core add for remove",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-remove-success",
        "workspace core add for remove",
        &stderr,
    )
    .unwrap();

    let mut add_tools = ql_command(&workspace_root);
    add_tools.args([
        "project",
        "add",
        &project_root.to_string_lossy(),
        "--name",
        "tools",
        "--dependency",
        "app",
        "--dependency",
        "core",
    ]);
    let output = run_command_capture(&mut add_tools, "`ql project add` tools for remove");
    let (_stdout, stderr) = expect_success(
        "project-remove-success",
        "workspace tools add for remove",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-remove-success",
        "workspace tools add for remove",
        &stderr,
    )
    .unwrap();

    let mut remove = ql_command(&workspace_root);
    remove.args([
        "project",
        "remove",
        &request_path.to_string_lossy(),
        "--name",
        "tools",
    ]);
    let output = run_command_capture(
        &mut remove,
        "`ql project remove` workspace member source path",
    );
    let (stdout, stderr) =
        expect_success("project-remove-success", "remove workspace member", &output).unwrap();
    expect_empty_stderr("project-remove-success", "remove workspace member", &stderr).unwrap();
    expect_stdout_contains_all(
        "project-remove-success",
        &stdout.replace('\\', "/"),
        &[
            &format!(
                "updated: {}",
                project_root
                    .join("qlang.toml")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            &format!(
                "removed: {}",
                removed_member_root.to_string_lossy().replace('\\', "/")
            ),
        ],
    )
    .unwrap();

    let workspace_manifest = read_normalized_file(
        &project_root.join("qlang.toml"),
        "workspace manifest after remove",
    );
    assert!(
        workspace_manifest.contains("packages/app"),
        "workspace manifest should keep existing members after remove"
    );
    assert!(
        workspace_manifest.contains("packages/core"),
        "workspace manifest should keep unrelated members after remove"
    );
    assert!(
        !workspace_manifest.contains("packages/tools"),
        "workspace manifest should drop the removed member entry"
    );
    assert!(
        removed_member_root.is_dir(),
        "project remove should keep the removed member files on disk"
    );

    let mut graph = ql_command(&workspace_root);
    graph.args(["project", "graph", &project_root.to_string_lossy()]);
    let output = run_command_capture(&mut graph, "`ql project graph` after remove");
    let (stdout, stderr) = expect_success(
        "project-remove-success",
        "graph workspace after remove",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-remove-success",
        "graph workspace after remove",
        &stderr,
    )
    .unwrap();
    let normalized_stdout = stdout.replace('\\', "/");
    expect_stdout_contains_all(
        "project-remove-success",
        &normalized_stdout,
        &[
            "workspace_members:",
            "  - packages/app",
            "  - packages/core",
        ],
    )
    .unwrap();
    assert!(
        !normalized_stdout.contains("packages/tools"),
        "workspace graph should not include the removed member, got:\n{stdout}"
    );
}

#[test]
fn project_remove_cascade_updates_dependents_and_workspace_members() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-remove-cascade");
    let project_root = temp.path().join("workspace");
    let request_path = project_root.join("packages/core/src/main.ql");
    let removed_member_root = project_root.join("packages/core");

    let mut init = ql_command(&workspace_root);
    init.args([
        "project",
        "init",
        &project_root.to_string_lossy(),
        "--workspace",
        "--name",
        "app",
    ]);
    let output = run_command_capture(&mut init, "`ql project init` workspace for cascade remove");
    let (_stdout, stderr) = expect_success(
        "project-remove-cascade",
        "workspace init for cascade remove",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-remove-cascade",
        "workspace init for cascade remove",
        &stderr,
    )
    .unwrap();

    let mut add_core = ql_command(&workspace_root);
    add_core.args([
        "project",
        "add",
        &project_root.to_string_lossy(),
        "--name",
        "core",
    ]);
    let output = run_command_capture(&mut add_core, "`ql project add` core for cascade remove");
    let (_stdout, stderr) = expect_success(
        "project-remove-cascade",
        "workspace core add for cascade remove",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-remove-cascade",
        "workspace core add for cascade remove",
        &stderr,
    )
    .unwrap();

    let mut add_tools = ql_command(&workspace_root);
    add_tools.args([
        "project",
        "add",
        &project_root.to_string_lossy(),
        "--name",
        "tools",
        "--dependency",
        "core",
    ]);
    let output = run_command_capture(&mut add_tools, "`ql project add` tools for cascade remove");
    let (_stdout, stderr) = expect_success(
        "project-remove-cascade",
        "workspace tools add for cascade remove",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-remove-cascade",
        "workspace tools add for cascade remove",
        &stderr,
    )
    .unwrap();

    let mut remove = ql_command(&workspace_root);
    remove.args([
        "project",
        "remove",
        &request_path.to_string_lossy(),
        "--name",
        "core",
        "--cascade",
    ]);
    let output = run_command_capture(
        &mut remove,
        "`ql project remove --cascade` workspace member with dependents",
    );
    let (stdout, stderr) = expect_success(
        "project-remove-cascade",
        "remove workspace member with cascading dependency cleanup",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-remove-cascade",
        "remove workspace member with cascading dependency cleanup",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "project-remove-cascade",
        &stdout.replace('\\', "/"),
        &[
            &format!(
                "updated: {}",
                project_root
                    .join("qlang.toml")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            &format!(
                "updated: {}",
                project_root
                    .join("packages/tools/qlang.toml")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            &format!(
                "removed: {}",
                removed_member_root.to_string_lossy().replace('\\', "/")
            ),
        ],
    )
    .unwrap();

    let workspace_manifest = read_normalized_file(
        &project_root.join("qlang.toml"),
        "workspace manifest after cascade remove",
    );
    assert!(
        workspace_manifest.contains("packages/app"),
        "workspace manifest should keep unrelated members after cascade remove"
    );
    assert!(
        workspace_manifest.contains("packages/tools"),
        "workspace manifest should keep dependents after cascade remove"
    );
    assert!(
        !workspace_manifest.contains("packages/core"),
        "workspace manifest should drop the removed member entry after cascade remove"
    );
    assert_eq!(
        read_normalized_file(
            &project_root.join("packages/tools/qlang.toml"),
            "dependent manifest after cascade remove"
        ),
        "[package]\nname = \"tools\"\n"
    );
    assert!(
        removed_member_root.is_dir(),
        "project remove --cascade should keep the removed member files on disk"
    );
}
