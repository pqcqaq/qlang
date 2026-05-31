use std::path::{Path, PathBuf};

use serde_json::Value as JsonValue;

use super::{TempDir, executable_output_path, expect_stdout_contains_all};

pub struct PackageProfileFixture {
    pub temp: TempDir,
    pub project_root: PathBuf,
    pub smoke_path: PathBuf,
    pub debug_smoke_output: PathBuf,
    pub release_smoke_output: PathBuf,
}

pub fn write_package_profile_fixture(prefix: &str, default_profile: &str) -> PackageProfileFixture {
    let temp = TempDir::new(prefix);
    let project_root = temp.path().join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create package source root for profile test");
    let manifest = format!(
        r#"
[package]
name = "app"

[profile]
default = "{default_profile}"
"#,
    );
    temp.write("app/qlang.toml", &manifest);
    temp.write("app/src/lib.ql", "pub fn helper() -> Int { return 1 }\n");
    let smoke_path = temp.write("app/tests/smoke.ql", "fn main() -> Int { return 0 }\n");

    let debug_smoke_output =
        executable_output_path(&project_root.join("target/ql/debug/tests"), "smoke");
    let release_smoke_output =
        executable_output_path(&project_root.join("target/ql/release/tests"), "smoke");

    PackageProfileFixture {
        temp,
        project_root,
        smoke_path,
        debug_smoke_output,
        release_smoke_output,
    }
}

pub fn write_package_default_release_profile_fixture(prefix: &str) -> PackageProfileFixture {
    write_package_profile_fixture(prefix, "release")
}

pub fn write_package_default_debug_profile_fixture(prefix: &str) -> PackageProfileFixture {
    write_package_profile_fixture(prefix, "debug")
}

pub fn expected_package_profile_test_json(
    request_path: &Path,
    requested_profile: &str,
    profile_overridden: bool,
    target_profile: &str,
) -> JsonValue {
    expected_profile_test_json(
        request_path,
        "tests/smoke.ql",
        requested_profile,
        profile_overridden,
        target_profile,
        false,
    )
}

pub fn expected_package_profile_test_listing_json(
    request_path: &Path,
    requested_profile: &str,
    profile_overridden: bool,
    target_profile: &str,
) -> JsonValue {
    expected_profile_test_json(
        request_path,
        "tests/smoke.ql",
        requested_profile,
        profile_overridden,
        target_profile,
        true,
    )
}

fn expected_profile_test_json(
    request_path: &Path,
    target_path: &str,
    requested_profile: &str,
    profile_overridden: bool,
    target_profile: &str,
    list_only: bool,
) -> JsonValue {
    serde_json::json!({
        "schema": "ql.test.v1",
        "path": request_path.display().to_string().replace('\\', "/"),
        "requested_profile": requested_profile,
        "profile_overridden": profile_overridden,
        "package_name": JsonValue::Null,
        "filter": JsonValue::Null,
        "list_only": list_only,
        "status": if list_only { "listed" } else { "ok" },
        "discovered_total": 1,
        "selected_total": 1,
        "targets": [
            {
                "path": target_path,
                "kind": "smoke",
                "profile": target_profile,
            }
        ],
        "passed": if list_only { 0 } else { 1 },
        "failed": 0,
        "failures": [],
    })
}

pub struct WorkspaceProfileFixture {
    pub temp: TempDir,
    pub project_root: PathBuf,
    pub smoke_path: PathBuf,
    pub debug_smoke_output: PathBuf,
    pub release_smoke_output: PathBuf,
    pub other_release_output: Option<PathBuf>,
}

pub fn write_workspace_profile_fixture(
    prefix: &str,
    default_profile: &str,
    include_other_test: bool,
) -> WorkspaceProfileFixture {
    let temp = TempDir::new(prefix);
    let project_root = temp.path().join("workspace");
    std::fs::create_dir_all(project_root.join("packages/app/src"))
        .expect("create workspace package source tree for workspace profile test");
    let manifest = format!(
        r#"
[workspace]
members = ["packages/app"]

[profile]
default = "{default_profile}"
"#,
    );
    temp.write("workspace/qlang.toml", &manifest);
    temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace/packages/app/src/lib.ql",
        "pub fn helper() -> Int { return 1 }\n",
    );
    let smoke_path = temp.write(
        "workspace/packages/app/tests/smoke.ql",
        "fn main() -> Int { return 0 }\n",
    );
    let other_release_output = if include_other_test {
        temp.write(
            "workspace/packages/app/tests/other.ql",
            "fn main() -> Int { return 0 }\n",
        );
        Some(executable_output_path(
            &project_root.join("packages/app/target/ql/release/tests"),
            "other",
        ))
    } else {
        None
    };

    let debug_smoke_output = executable_output_path(
        &project_root.join("packages/app/target/ql/debug/tests"),
        "smoke",
    );
    let release_smoke_output = executable_output_path(
        &project_root.join("packages/app/target/ql/release/tests"),
        "smoke",
    );

    WorkspaceProfileFixture {
        temp,
        project_root,
        smoke_path,
        debug_smoke_output,
        release_smoke_output,
        other_release_output,
    }
}

pub fn write_workspace_default_release_profile_fixture(prefix: &str) -> WorkspaceProfileFixture {
    write_workspace_profile_fixture(prefix, "release", false)
}

pub fn write_workspace_default_debug_profile_fixture(prefix: &str) -> WorkspaceProfileFixture {
    write_workspace_profile_fixture(prefix, "debug", false)
}

pub fn write_workspace_member_file_profile_fixture(prefix: &str) -> WorkspaceProfileFixture {
    write_workspace_profile_fixture(prefix, "release", true)
}

pub fn expected_workspace_profile_test_json(
    request_path: &Path,
    requested_profile: &str,
    profile_overridden: bool,
    target_profile: &str,
) -> JsonValue {
    expected_profile_test_json(
        request_path,
        "packages/app/tests/smoke.ql",
        requested_profile,
        profile_overridden,
        target_profile,
        false,
    )
}

pub fn expected_workspace_profile_test_listing_json(
    request_path: &Path,
    requested_profile: &str,
    profile_overridden: bool,
    target_profile: &str,
) -> JsonValue {
    expected_profile_test_json(
        request_path,
        "packages/app/tests/smoke.ql",
        requested_profile,
        profile_overridden,
        target_profile,
        true,
    )
}

pub fn expect_profile_stdout(case_name: &str, stdout: &str, target_line: &str) {
    expect_stdout_contains_all(
        case_name,
        &stdout.replace('\\', "/"),
        &[target_line, "test result: ok. 1 passed; 0 failed"],
    )
    .unwrap_or_else(|error| panic!("{error}"));
}

fn normalize_output_text(text: &str) -> String {
    text.replace("\r\n", "\n")
}

fn parse_json_output(case_name: &str, stdout: &str) -> JsonValue {
    serde_json::from_str(&normalize_output_text(stdout))
        .unwrap_or_else(|error| panic!("[{case_name}] parse json stdout: {error}\n{stdout}"))
}

pub fn assert_profile_json(
    case_name: &str,
    stdout: &str,
    expected: JsonValue,
    failure_message: &str,
) {
    let actual = parse_json_output(case_name, stdout);
    assert_eq!(actual, expected, "{failure_message}");
}

pub fn assert_package_profile_artifacts_absent(fixture: &PackageProfileFixture, context: &str) {
    assert!(
        !fixture.debug_smoke_output.exists() && !fixture.release_smoke_output.exists(),
        "{context} should not emit test artifacts"
    );
}

pub fn assert_workspace_profile_artifacts_absent(fixture: &WorkspaceProfileFixture, context: &str) {
    assert!(
        !fixture.debug_smoke_output.exists() && !fixture.release_smoke_output.exists(),
        "{context} should not emit test artifacts"
    );
}
