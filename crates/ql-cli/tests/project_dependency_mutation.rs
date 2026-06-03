mod support;

use std::path::PathBuf;
use std::process::Stdio;

use support::{
    TempDir, assert_no_atomic_write_temp_files, assert_no_build_lock_directories,
    expect_empty_stderr, expect_stdout_contains_all, expect_success, ql_command,
    read_normalized_file, run_command_capture, workspace_root,
};

struct AppCoreWorkspaceFixture {
    app_manifest_path: PathBuf,
    app_member_dir: PathBuf,
}

fn write_app_core_workspace_fixture(
    temp: &TempDir,
    app_manifest_source: &str,
) -> AppCoreWorkspaceFixture {
    let project_root = temp.path().join("workspace");
    let app_member_dir = project_root.join("packages/app");
    temp.write(
        "workspace/qlang.toml",
        "[workspace]\nmembers = [\"packages/app\", \"packages/core\"]\n",
    );
    let app_manifest_path = temp.write("workspace/packages/app/qlang.toml", app_manifest_source);
    temp.write(
        "workspace/packages/core/qlang.toml",
        "[package]\nname = \"core\"\n",
    );
    temp.write(
        "workspace/packages/app/src/main.ql",
        "fn main() -> Int {\n    return 0\n}\n",
    );
    temp.write(
        "workspace/packages/core/src/lib.ql",
        "pub fn core() -> Int {\n    return 1\n}\n",
    );

    AppCoreWorkspaceFixture {
        app_manifest_path,
        app_member_dir,
    }
}

#[test]
fn project_add_dependency_updates_existing_package_manifest_from_member_source_path() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-add-dependency-success");
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
    let output = run_command_capture(&mut init, "`ql project init` workspace for add-dependency");
    let (_stdout, stderr) = expect_success(
        "project-add-dependency-success",
        "workspace init for add-dependency",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-add-dependency-success",
        "workspace init for add-dependency",
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
    let output = run_command_capture(
        &mut add_core,
        "`ql project add` workspace member for add-dependency",
    );
    let (_stdout, stderr) = expect_success(
        "project-add-dependency-success",
        "add workspace member for add-dependency",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-add-dependency-success",
        "add workspace member for add-dependency",
        &stderr,
    )
    .unwrap();

    let mut add_dependency = ql_command(&workspace_root);
    add_dependency.args([
        "project",
        "add-dependency",
        &request_path.to_string_lossy(),
        "--name",
        "core",
    ]);
    let output = run_command_capture(
        &mut add_dependency,
        "`ql project add-dependency` workspace member source path",
    );
    let (stdout, stderr) = expect_success(
        "project-add-dependency-success",
        "add dependency to existing package manifest",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-add-dependency-success",
        "add dependency to existing package manifest",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "project-add-dependency-success",
        &stdout,
        &[&format!(
            "updated: {}",
            project_root
                .join("packages/app/qlang.toml")
                .to_string_lossy()
                .replace('\\', "/")
        )],
    )
    .unwrap();

    assert_eq!(
        read_normalized_file(
            &project_root.join("packages/app/qlang.toml"),
            "workspace member manifest after add-dependency"
        ),
        "[dependencies]\ncore = \"../core\"\n\n[package]\nname = \"app\"\n"
    );
}

#[test]
fn project_dependency_updates_serialize_concurrent_manifest_writes() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-dependency-concurrent-writes");
    let project_root = temp.path().join("workspace");
    let app_manifest_path = project_root.join("packages/app/qlang.toml");

    temp.write(
        "workspace/qlang.toml",
        "[workspace]\nmembers = [\"packages/app\", \"packages/core\", \"packages/util\"]\n",
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        "[package]\nname = \"app\"\n\n[dependencies]\ncore = \"../core\"\n",
    );
    temp.write(
        "workspace/packages/core/qlang.toml",
        "[package]\nname = \"core\"\n",
    );
    temp.write(
        "workspace/packages/util/qlang.toml",
        "[package]\nname = \"util\"\n",
    );

    let mut children = Vec::new();

    let mut add_util = ql_command(&workspace_root);
    add_util.current_dir(temp.path());
    add_util.args([
        "project",
        "add-dependency",
        &project_root.to_string_lossy(),
        "--package",
        "app",
        "--name",
        "util",
    ]);
    add_util.stdout(Stdio::piped()).stderr(Stdio::piped());
    let add_child = add_util
        .spawn()
        .unwrap_or_else(|error| panic!("spawn concurrent ql project add-dependency: {error}"));
    children.push(("add util", add_child));

    let mut remove_core = ql_command(&workspace_root);
    remove_core.current_dir(temp.path());
    remove_core.args([
        "project",
        "remove-dependency",
        &project_root.to_string_lossy(),
        "--package",
        "app",
        "--name",
        "core",
    ]);
    remove_core.stdout(Stdio::piped()).stderr(Stdio::piped());
    let remove_child = remove_core
        .spawn()
        .unwrap_or_else(|error| panic!("spawn concurrent ql project remove-dependency: {error}"));
    children.push(("remove core", remove_child));

    for (action, child) in children {
        let output = child.wait_with_output().unwrap_or_else(|error| {
            panic!("wait for concurrent ql project dependency edit `{action}`: {error}")
        });
        let (stdout, stderr) = expect_success(
            "project-dependency-concurrent-writes",
            &format!("concurrent dependency edit `{action}`"),
            &output,
        )
        .unwrap_or_else(|message| panic!("{message}"));
        expect_empty_stderr(
            "project-dependency-concurrent-writes",
            &format!("concurrent dependency edit `{action}`"),
            &stderr,
        )
        .unwrap_or_else(|message| panic!("{message}"));
        expect_stdout_contains_all(
            "project-dependency-concurrent-writes",
            &stdout.replace('\\', "/"),
            &[&format!(
                "updated: {}",
                app_manifest_path.to_string_lossy().replace('\\', "/")
            )],
        )
        .unwrap_or_else(|message| panic!("{message}"));
    }

    let actual = read_normalized_file(
        &app_manifest_path,
        "workspace member manifest after concurrent dependency edits",
    );
    assert!(
        !actual.contains("core = \"../core\""),
        "concurrent dependency edit should remove `core`: {actual}"
    );
    assert!(
        actual.contains("util = \"../util\""),
        "concurrent dependency edit should keep added `util`: {actual}"
    );
    assert_no_build_lock_directories("project-dependency-concurrent-writes", &project_root);
    assert_no_atomic_write_temp_files("project-dependency-concurrent-writes", &project_root);
}

#[test]
fn project_add_dependency_updates_existing_package_manifest_from_member_directory() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-add-dependency-member-dir");
    let fixture = write_app_core_workspace_fixture(&temp, "[package]\nname = \"app\"\n");

    let mut add_dependency = ql_command(&workspace_root);
    add_dependency.args([
        "project",
        "add-dependency",
        &fixture.app_member_dir.to_string_lossy(),
        "--name",
        "core",
    ]);
    let output = run_command_capture(
        &mut add_dependency,
        "`ql project add-dependency` workspace member directory",
    );
    let (stdout, stderr) = expect_success(
        "project-add-dependency-member-dir",
        "add dependency to existing package manifest from member directory",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-add-dependency-member-dir",
        "add dependency to existing package manifest from member directory",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "project-add-dependency-member-dir",
        &stdout.replace('\\', "/"),
        &[&format!(
            "updated: {}",
            fixture
                .app_manifest_path
                .to_string_lossy()
                .replace('\\', "/")
        )],
    )
    .unwrap();

    assert_eq!(
        read_normalized_file(
            &fixture.app_manifest_path,
            "workspace member manifest after member directory add-dependency"
        ),
        "[dependencies]\ncore = \"../core\"\n\n[package]\nname = \"app\"\n"
    );
}

#[test]
fn project_add_dependency_supports_workspace_root_package_selector() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-add-dependency-selector");
    let project_root = temp.path().join("workspace");

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
        "`ql project init` workspace for selected add-dependency",
    );
    let (_stdout, stderr) = expect_success(
        "project-add-dependency-selector",
        "workspace init for selected add-dependency",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-add-dependency-selector",
        "workspace init for selected add-dependency",
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
    let output = run_command_capture(
        &mut add_core,
        "`ql project add` workspace member for selected add-dependency",
    );
    let (_stdout, stderr) = expect_success(
        "project-add-dependency-selector",
        "add workspace member for selected add-dependency",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-add-dependency-selector",
        "add workspace member for selected add-dependency",
        &stderr,
    )
    .unwrap();

    let mut add_dependency = ql_command(&workspace_root);
    add_dependency.args([
        "project",
        "add-dependency",
        &project_root.to_string_lossy(),
        "--package",
        "app",
        "--name",
        "core",
    ]);
    let output = run_command_capture(
        &mut add_dependency,
        "`ql project add-dependency --package` workspace root",
    );
    let (stdout, stderr) = expect_success(
        "project-add-dependency-selector",
        "add dependency from workspace root with package selector",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-add-dependency-selector",
        "add dependency from workspace root with package selector",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "project-add-dependency-selector",
        &stdout,
        &[&format!(
            "updated: {}",
            project_root
                .join("packages/app/qlang.toml")
                .to_string_lossy()
                .replace('\\', "/")
        )],
    )
    .unwrap();

    assert_eq!(
        read_normalized_file(
            &project_root.join("packages/app/qlang.toml"),
            "workspace member manifest after selected add-dependency"
        ),
        "[dependencies]\ncore = \"../core\"\n\n[package]\nname = \"app\"\n"
    );
}

#[test]
fn project_add_dependency_supports_external_local_path() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-add-dependency-path");
    let project_root = temp.path().join("workspace");
    let vendor_source_path = project_root.join("vendor/core/src/lib.ql");

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
        "`ql project init` workspace for path add-dependency",
    );
    let (_stdout, stderr) = expect_success(
        "project-add-dependency-path",
        "workspace init for path add-dependency",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-add-dependency-path",
        "workspace init for path add-dependency",
        &stderr,
    )
    .unwrap();

    temp.write(
        "workspace/vendor/core/qlang.toml",
        "[package]\nname = \"vendor.core\"\n",
    );
    temp.write(
        "workspace/vendor/core/src/lib.ql",
        "pub fn helper() -> Int {\n    return 1\n}\n",
    );

    let mut add_dependency = ql_command(&workspace_root);
    add_dependency.args([
        "project",
        "add-dependency",
        &project_root.to_string_lossy(),
        "--package",
        "app",
        "--path",
        &vendor_source_path.to_string_lossy(),
    ]);
    let output = run_command_capture(
        &mut add_dependency,
        "`ql project add-dependency --path` external local package",
    );
    let (stdout, stderr) = expect_success(
        "project-add-dependency-path",
        "add external local dependency by path",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-add-dependency-path",
        "add external local dependency by path",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "project-add-dependency-path",
        &stdout,
        &[&format!(
            "updated: {}",
            project_root
                .join("packages/app/qlang.toml")
                .to_string_lossy()
                .replace('\\', "/")
        )],
    )
    .unwrap();

    assert_eq!(
        read_normalized_file(
            &project_root.join("packages/app/qlang.toml"),
            "workspace member manifest after path add-dependency"
        ),
        "[dependencies]\n\"vendor.core\" = \"../../vendor/core\"\n\n[package]\nname = \"app\"\n"
    );
}

#[test]
fn project_remove_dependency_updates_existing_package_manifest() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-remove-dependency-success");
    let project_root = temp.path().join("workspace");
    let request_path = project_root.join("packages/app/src/main.ql");

    temp.write(
        "workspace/qlang.toml",
        "[workspace]\nmembers = [\"packages/app\", \"packages/core\"]\n",
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        "[package]\nname = \"app\"\n\n[dependencies]\ncore = \"../core\"\n",
    );
    temp.write(
        "workspace/packages/core/qlang.toml",
        "[package]\nname = \"core\"\n",
    );
    temp.write(
        "workspace/packages/app/src/main.ql",
        "fn main() -> Int {\n    return 0\n}\n",
    );

    let mut remove_dependency = ql_command(&workspace_root);
    remove_dependency.args([
        "project",
        "remove-dependency",
        &request_path.to_string_lossy(),
        "--name",
        "core",
    ]);
    let output = run_command_capture(
        &mut remove_dependency,
        "`ql project remove-dependency` existing package manifest",
    );
    let (stdout, stderr) = expect_success(
        "project-remove-dependency-success",
        "remove dependency from existing package manifest",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-remove-dependency-success",
        "remove dependency from existing package manifest",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "project-remove-dependency-success",
        &stdout,
        &[&format!(
            "updated: {}",
            project_root
                .join("packages/app/qlang.toml")
                .to_string_lossy()
                .replace('\\', "/")
        )],
    )
    .unwrap();

    assert_eq!(
        read_normalized_file(
            &project_root.join("packages/app/qlang.toml"),
            "workspace member manifest after remove-dependency"
        ),
        "[package]\nname = \"app\"\n"
    );
}

#[test]
fn project_remove_dependency_updates_existing_package_manifest_from_member_directory() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-remove-dependency-member-dir");
    let fixture = write_app_core_workspace_fixture(
        &temp,
        "[package]\nname = \"app\"\n\n[dependencies]\ncore = \"../core\"\n",
    );

    let mut remove_dependency = ql_command(&workspace_root);
    remove_dependency.args([
        "project",
        "remove-dependency",
        &fixture.app_member_dir.to_string_lossy(),
        "--name",
        "core",
    ]);
    let output = run_command_capture(
        &mut remove_dependency,
        "`ql project remove-dependency` workspace member directory",
    );
    let (stdout, stderr) = expect_success(
        "project-remove-dependency-member-dir",
        "remove dependency from existing package manifest from member directory",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-remove-dependency-member-dir",
        "remove dependency from existing package manifest from member directory",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "project-remove-dependency-member-dir",
        &stdout.replace('\\', "/"),
        &[&format!(
            "updated: {}",
            fixture
                .app_manifest_path
                .to_string_lossy()
                .replace('\\', "/")
        )],
    )
    .unwrap();

    assert_eq!(
        read_normalized_file(
            &fixture.app_manifest_path,
            "workspace member manifest after member directory remove-dependency"
        ),
        "[package]\nname = \"app\"\n"
    );
}

#[test]
fn project_remove_dependency_supports_workspace_root_package_selector() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-remove-dependency-selector");
    let project_root = temp.path().join("workspace");

    temp.write(
        "workspace/qlang.toml",
        "[workspace]\nmembers = [\"packages/app\", \"packages/core\"]\n",
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        "[package]\nname = \"app\"\n\n[dependencies]\ncore = \"../core\"\n",
    );
    temp.write(
        "workspace/packages/core/qlang.toml",
        "[package]\nname = \"core\"\n",
    );

    let mut remove_dependency = ql_command(&workspace_root);
    remove_dependency.args([
        "project",
        "remove-dependency",
        &project_root.to_string_lossy(),
        "--package",
        "app",
        "--name",
        "core",
    ]);
    let output = run_command_capture(
        &mut remove_dependency,
        "`ql project remove-dependency --package` workspace root",
    );
    let (stdout, stderr) = expect_success(
        "project-remove-dependency-selector",
        "remove dependency from workspace root with package selector",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-remove-dependency-selector",
        "remove dependency from workspace root with package selector",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "project-remove-dependency-selector",
        &stdout,
        &[&format!(
            "updated: {}",
            project_root
                .join("packages/app/qlang.toml")
                .to_string_lossy()
                .replace('\\', "/")
        )],
    )
    .unwrap();

    assert_eq!(
        read_normalized_file(
            &project_root.join("packages/app/qlang.toml"),
            "workspace member manifest after selected remove-dependency"
        ),
        "[package]\nname = \"app\"\n"
    );
}

#[test]
fn project_remove_dependency_removes_legacy_reference_entry() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-remove-dependency-legacy");
    let project_root = temp.path().join("workspace");
    let request_path = project_root.join("packages/app/src/main.ql");

    temp.write(
        "workspace/qlang.toml",
        "[workspace]\nmembers = [\"packages/app\", \"packages/core\"]\n",
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        "[package]\nname = \"app\"\n\n[references]\npackages = [\"../core\"]\n",
    );
    temp.write(
        "workspace/packages/core/qlang.toml",
        "[package]\nname = \"core\"\n",
    );
    temp.write(
        "workspace/packages/app/src/main.ql",
        "fn main() -> Int {\n    return 0\n}\n",
    );

    let mut remove_dependency = ql_command(&workspace_root);
    remove_dependency.args([
        "project",
        "remove-dependency",
        &request_path.to_string_lossy(),
        "--name",
        "core",
    ]);
    let output = run_command_capture(
        &mut remove_dependency,
        "`ql project remove-dependency` legacy reference entry",
    );
    let (stdout, stderr) = expect_success(
        "project-remove-dependency-legacy",
        "remove legacy reference dependency from existing package manifest",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-remove-dependency-legacy",
        "remove legacy reference dependency from existing package manifest",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "project-remove-dependency-legacy",
        &stdout,
        &[&format!(
            "updated: {}",
            project_root
                .join("packages/app/qlang.toml")
                .to_string_lossy()
                .replace('\\', "/")
        )],
    )
    .unwrap();

    assert_eq!(
        read_normalized_file(
            &project_root.join("packages/app/qlang.toml"),
            "workspace member manifest after legacy remove-dependency"
        ),
        "[package]\nname = \"app\"\n"
    );
}

#[test]
fn project_remove_dependency_all_updates_all_workspace_dependents() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-remove-dependency-all");
    let project_root = temp.path().join("workspace");

    temp.write(
        "workspace/qlang.toml",
        "[workspace]\nmembers = [\"packages/app\", \"packages/tools\", \"packages/core\"]\n",
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        "[package]\nname = \"app\"\n\n[dependencies]\ncore = \"../core\"\n",
    );
    temp.write(
        "workspace/packages/tools/qlang.toml",
        "[package]\nname = \"tools\"\n\n[references]\npackages = [\"../core\"]\n",
    );
    temp.write(
        "workspace/packages/core/qlang.toml",
        "[package]\nname = \"core\"\n",
    );

    let mut remove_dependency = ql_command(&workspace_root);
    remove_dependency.args([
        "project",
        "remove-dependency",
        &project_root.to_string_lossy(),
        "--name",
        "core",
        "--all",
    ]);
    let output = run_command_capture(
        &mut remove_dependency,
        "`ql project remove-dependency --all` workspace dependents",
    );
    let (stdout, stderr) = expect_success(
        "project-remove-dependency-all",
        "remove dependency from all workspace dependents",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-remove-dependency-all",
        "remove dependency from all workspace dependents",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "project-remove-dependency-all",
        &stdout.replace('\\', "/"),
        &[
            &format!(
                "updated: {}",
                project_root
                    .join("packages/app/qlang.toml")
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
        ],
    )
    .unwrap();

    assert_eq!(
        read_normalized_file(
            &project_root.join("packages/app/qlang.toml"),
            "workspace app manifest after remove-dependency --all"
        ),
        "[package]\nname = \"app\"\n"
    );
    assert_eq!(
        read_normalized_file(
            &project_root.join("packages/tools/qlang.toml"),
            "workspace tools manifest after remove-dependency --all"
        ),
        "[package]\nname = \"tools\"\n"
    );
}

#[test]
fn project_remove_dependency_all_derives_package_name_from_member_source_path() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-remove-dependency-all-derived-name");
    let project_root = temp.path().join("workspace");
    let request_path = project_root.join("packages/core/src/main.ql");

    temp.write(
        "workspace/qlang.toml",
        "[workspace]\nmembers = [\"packages/app\", \"packages/tools\", \"packages/core\"]\n",
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        "[package]\nname = \"app\"\n\n[dependencies]\ncore = \"../core\"\n",
    );
    temp.write(
        "workspace/packages/tools/qlang.toml",
        "[package]\nname = \"tools\"\n\n[references]\npackages = [\"../core\"]\n",
    );
    temp.write(
        "workspace/packages/core/qlang.toml",
        "[package]\nname = \"core\"\n",
    );
    temp.write(
        "workspace/packages/core/src/main.ql",
        "fn main() -> Int {\n    return 0\n}\n",
    );

    let mut remove_dependency = ql_command(&workspace_root);
    remove_dependency.args([
        "project",
        "remove-dependency",
        &request_path.to_string_lossy(),
        "--all",
    ]);
    let output = run_command_capture(
        &mut remove_dependency,
        "`ql project remove-dependency --all` derived package name",
    );
    let (stdout, stderr) = expect_success(
        "project-remove-dependency-all-derived-name",
        "remove dependency from all workspace dependents with derived package name",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-remove-dependency-all-derived-name",
        "remove dependency from all workspace dependents with derived package name",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "project-remove-dependency-all-derived-name",
        &stdout.replace('\\', "/"),
        &[
            &format!(
                "updated: {}",
                project_root
                    .join("packages/app/qlang.toml")
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
        ],
    )
    .unwrap();

    assert_eq!(
        read_normalized_file(
            &project_root.join("packages/app/qlang.toml"),
            "workspace app manifest after derived remove-dependency --all"
        ),
        "[package]\nname = \"app\"\n"
    );
    assert_eq!(
        read_normalized_file(
            &project_root.join("packages/tools/qlang.toml"),
            "workspace tools manifest after derived remove-dependency --all"
        ),
        "[package]\nname = \"tools\"\n"
    );
}
