use std::fs;
use std::path::{Path, PathBuf};

use ql_driver::{ToolchainOptions, discover_toolchain};
use serde_json::Value as JsonValue;

use super::TempDir;

pub fn toolchain_available(context: &str) -> bool {
    let Ok(_toolchain) = discover_toolchain(&ToolchainOptions::default()) else {
        eprintln!(
            "skipping {context}: no clang-style compiler found via ql-driver toolchain discovery"
        );
        return false;
    };
    true
}

pub fn parse_json_output(case_name: &str, stdout: &str) -> JsonValue {
    serde_json::from_str(&stdout.replace("\r\n", "\n"))
        .unwrap_or_else(|error| panic!("[{case_name}] parse json stdout: {error}\n{stdout}"))
}

pub fn json_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

fn repo_stdlib_checked_files(stdlib_root: &Path) -> Vec<PathBuf> {
    vec![
        stdlib_root.join("packages/core/src/lib.ql"),
        stdlib_root.join("packages/option/src/lib.ql"),
        stdlib_root.join("packages/result/src/lib.ql"),
        stdlib_root.join("packages/array/src/lib.ql"),
        stdlib_root.join("packages/test/src/lib.ql"),
        stdlib_root.join("examples/starter/src/lib.ql"),
        stdlib_root.join("examples/starter/src/main.ql"),
    ]
}

pub fn repo_stdlib_written_interfaces(stdlib_root: &Path) -> Vec<PathBuf> {
    vec![
        stdlib_root.join("packages/option/std.option.qi"),
        stdlib_root.join("packages/core/std.core.qi"),
        stdlib_root.join("packages/array/std.array.qi"),
        stdlib_root.join("packages/result/std.result.qi"),
        stdlib_root.join("packages/test/std.test.qi"),
    ]
}

fn repo_stdlib_loaded_interfaces(stdlib_root: &Path) -> Vec<PathBuf> {
    vec![
        stdlib_root.join("packages/option/std.option.qi"),
        stdlib_root.join("packages/core/std.core.qi"),
        stdlib_root.join("packages/array/std.array.qi"),
        stdlib_root.join("packages/core/std.core.qi"),
        stdlib_root.join("packages/option/std.option.qi"),
        stdlib_root.join("packages/result/std.result.qi"),
        stdlib_root.join("packages/array/std.array.qi"),
        stdlib_root.join("packages/core/std.core.qi"),
        stdlib_root.join("packages/option/std.option.qi"),
        stdlib_root.join("packages/result/std.result.qi"),
        stdlib_root.join("packages/test/std.test.qi"),
    ]
}

fn json_path_list(paths: &[PathBuf]) -> JsonValue {
    serde_json::json!(paths.iter().map(|path| json_path(path)).collect::<Vec<_>>())
}

fn normalize_cli_json_path(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    normalized
        .strip_prefix("//?/")
        .unwrap_or(&normalized)
        .to_owned()
}

pub fn assert_repo_stdlib_check_json(
    context: &str,
    check_json: &JsonValue,
    stdlib_root: &Path,
    sync_interfaces: bool,
    written_interfaces: &[PathBuf],
) {
    assert_eq!(check_json["schema"], "ql.check.v1");
    assert_eq!(check_json["scope"], "workspace");
    assert_eq!(check_json["status"], "ok");
    assert_eq!(
        check_json["project_manifest_path"],
        json_path(&stdlib_root.join("qlang.toml"))
    );
    assert_eq!(check_json["diagnostic_files"], serde_json::json!([]));
    assert_eq!(check_json["failing_manifests"], serde_json::json!([]));
    assert_eq!(check_json["sync_interfaces"], sync_interfaces);
    assert_eq!(
        check_json["checked_files"],
        json_path_list(&repo_stdlib_checked_files(stdlib_root)),
        "{context} should check every stdlib package/example source"
    );
    assert_eq!(
        check_json["loaded_interfaces"],
        json_path_list(&repo_stdlib_loaded_interfaces(stdlib_root)),
        "{context} should report loaded stdlib dependency interfaces in traversal order"
    );
    let actual_written_interfaces = check_json["written_interfaces"]
        .as_array()
        .unwrap_or_else(|| panic!("{context} should report written interface paths as an array"))
        .iter()
        .map(|path| {
            normalize_cli_json_path(
                path.as_str()
                    .unwrap_or_else(|| panic!("{context} should report string interface paths")),
            )
        })
        .collect::<Vec<_>>();
    let expected_written_interfaces = written_interfaces
        .iter()
        .map(|path| normalize_cli_json_path(&json_path(path)))
        .collect::<Vec<_>>();
    assert_eq!(
        actual_written_interfaces, expected_written_interfaces,
        "{context} should report synchronized interface artifacts"
    );
}

pub fn assert_repo_stdlib_starter_check_json(
    context: &str,
    check_json: &JsonValue,
    stdlib_root: &Path,
) {
    assert_eq!(check_json["schema"], "ql.check.v1");
    assert_eq!(check_json["scope"], "workspace");
    assert_eq!(check_json["status"], "ok");
    assert_eq!(
        check_json["project_manifest_path"],
        json_path(&stdlib_root.join("qlang.toml"))
    );
    assert_eq!(check_json["diagnostic_files"], serde_json::json!([]));
    assert_eq!(check_json["failing_manifests"], serde_json::json!([]));
    assert_eq!(check_json["sync_interfaces"], false);
    assert_eq!(check_json["written_interfaces"], serde_json::json!([]));
    assert_eq!(
        check_json["checked_files"],
        serde_json::json!([
            json_path(&stdlib_root.join("examples/starter/src/lib.ql")),
            json_path(&stdlib_root.join("examples/starter/src/main.ql")),
        ]),
        "{context} should check only the selected starter package sources"
    );
    assert_eq!(
        check_json["loaded_interfaces"],
        serde_json::json!([
            json_path(&stdlib_root.join("packages/array/std.array.qi")),
            json_path(&stdlib_root.join("packages/core/std.core.qi")),
            json_path(&stdlib_root.join("packages/option/std.option.qi")),
            json_path(&stdlib_root.join("packages/result/std.result.qi")),
            json_path(&stdlib_root.join("packages/test/std.test.qi")),
        ]),
        "{context} should load the starter dependency interfaces"
    );
}

pub fn assert_repo_stdlib_starter_status_json(
    context: &str,
    status_json: &JsonValue,
    stdlib_root: &Path,
) {
    assert_eq!(status_json["schema"], "ql.project.status.v1");
    assert_eq!(status_json["kind"], "workspace");
    assert_eq!(status_json["status"], "ok");
    assert_eq!(status_json["path"], json_path(stdlib_root));
    assert_eq!(
        status_json["project_manifest_path"],
        json_path(&stdlib_root.join("qlang.toml"))
    );

    let members = status_json["members"].as_array().unwrap_or_else(|| {
        panic!("{context} should expose selected workspace member: {status_json}")
    });
    assert_eq!(
        members.len(),
        1,
        "{context} should expose only the selected starter member"
    );
    let starter = &members[0];
    assert_eq!(starter["member"], "examples/starter");
    assert_eq!(starter["package_name"], "stdlib.starter");
    assert_eq!(
        starter["manifest_path"],
        json_path(&stdlib_root.join("examples/starter/qlang.toml"))
    );
    assert_eq!(
        starter["interface"]["path"],
        json_path(&stdlib_root.join("examples/starter/stdlib.starter.qi"))
    );
    assert_eq!(starter["interface"]["status"], "valid");
    assert_eq!(starter["interface"]["detail"], JsonValue::Null);
    assert_eq!(starter["interface"]["stale_reasons"], serde_json::json!([]));
    assert_eq!(
        starter["targets"],
        serde_json::json!([
            {
                "kind": "lib",
                "path": "src/lib.ql",
            },
            {
                "kind": "bin",
                "path": "src/main.ql",
            }
        ])
    );

    let dependencies = starter["dependencies"]
        .as_array()
        .unwrap_or_else(|| panic!("{context} should expose starter dependencies: {status_json}"));
    assert_eq!(
        dependencies.len(),
        5,
        "{context} should expose every starter dependency"
    );
    for (package_name, member, dependency_path) in [
        ("std.array", "packages/array", "../../packages/array"),
        ("std.core", "packages/core", "../../packages/core"),
        ("std.option", "packages/option", "../../packages/option"),
        ("std.result", "packages/result", "../../packages/result"),
        ("std.test", "packages/test", "../../packages/test"),
    ] {
        assert!(
            dependencies.iter().any(|actual| {
                actual["kind"] == "workspace"
                    && actual["package_name"] == package_name
                    && actual["member"] == member
                    && actual["dependency_path"] == dependency_path
                    && actual["manifest_path"]
                        == json_path(&stdlib_root.join(format!("{member}/qlang.toml")))
            }),
            "{context} should expose dependency `{package_name}`: {status_json}"
        );
    }
}

pub fn assert_repo_stdlib_starter_graph_json(
    context: &str,
    graph_json: &JsonValue,
    stdlib_root: &Path,
) {
    assert_eq!(graph_json["schema"], "ql.project.graph.v1");
    assert_eq!(
        graph_json["manifest_path"],
        json_path(&stdlib_root.join("examples/starter/qlang.toml"))
    );
    assert_eq!(graph_json["package_name"], "stdlib.starter");
    assert_eq!(graph_json["interface"]["path"], "stdlib.starter.qi");
    assert_eq!(graph_json["interface"]["status"], "valid");
    assert_eq!(graph_json["interface"]["detail"], JsonValue::Null);
    assert_eq!(
        graph_json["interface"]["stale_reasons"],
        serde_json::json!([])
    );
    assert_eq!(graph_json["workspace_members"], serde_json::json!([]));
    assert_eq!(graph_json["workspace_packages"], serde_json::json!([]));
    assert_eq!(
        graph_json["references"],
        serde_json::json!([
            "../../packages/array",
            "../../packages/core",
            "../../packages/option",
            "../../packages/result",
            "../../packages/test",
        ])
    );

    let reference_interfaces = graph_json["reference_interfaces"]
        .as_array()
        .unwrap_or_else(|| panic!("{context} should expose reference interfaces: {graph_json}"));
    assert_eq!(
        reference_interfaces.len(),
        5,
        "{context} should expose every starter reference interface"
    );
    for (package_name, reference, manifest_path, interface_path) in [
        (
            "std.array",
            "../../packages/array",
            "packages/array/qlang.toml",
            "packages/array/std.array.qi",
        ),
        (
            "std.core",
            "../../packages/core",
            "packages/core/qlang.toml",
            "packages/core/std.core.qi",
        ),
        (
            "std.option",
            "../../packages/option",
            "packages/option/qlang.toml",
            "packages/option/std.option.qi",
        ),
        (
            "std.result",
            "../../packages/result",
            "packages/result/qlang.toml",
            "packages/result/std.result.qi",
        ),
        (
            "std.test",
            "../../packages/test",
            "packages/test/qlang.toml",
            "packages/test/std.test.qi",
        ),
    ] {
        assert!(
            reference_interfaces.iter().any(|actual| {
                actual["package_name"] == package_name
                    && actual["reference"] == reference
                    && actual["manifest_path"] == manifest_path
                    && actual["path"] == interface_path
                    && actual["status"] == "valid"
                    && actual["detail"] == JsonValue::Null
                    && actual["stale_reasons"] == serde_json::json!([])
                    && actual["transitive_reference_failures"]["count"] == 0
                    && actual["transitive_reference_failures"]["first_failure"] == JsonValue::Null
            }),
            "{context} should expose valid starter reference interface `{package_name}`: {graph_json}"
        );
    }
}

pub fn assert_repo_stdlib_starter_targets_json(
    context: &str,
    targets_json: &JsonValue,
    stdlib_root: &Path,
) {
    assert_eq!(targets_json["schema"], "ql.project.targets.v1");
    assert_eq!(
        targets_json["members"],
        serde_json::json!([
            {
                "manifest_path": json_path(&stdlib_root.join("examples/starter/qlang.toml")),
                "package_name": "stdlib.starter",
                "targets": [
                    {
                        "kind": "lib",
                        "path": "src/lib.ql",
                    },
                    {
                        "kind": "bin",
                        "path": "src/main.ql",
                    }
                ],
            }
        ]),
        "{context} should expose only the selected starter targets"
    );
}

pub fn assert_repo_stdlib_starter_run_list_json(
    context: &str,
    targets_json: &JsonValue,
    stdlib_root: &Path,
) {
    assert_eq!(targets_json["schema"], "ql.project.targets.v1");
    assert_eq!(
        targets_json["members"],
        serde_json::json!([
            {
                "manifest_path": json_path(&stdlib_root.join("examples/starter/qlang.toml")),
                "package_name": "stdlib.starter",
                "targets": [
                    {
                        "kind": "bin",
                        "path": "src/main.ql",
                    }
                ],
            }
        ]),
        "{context} should expose only the selected starter runnable target"
    );
}

pub fn assert_repo_stdlib_test_list_json(
    context: &str,
    test_json: &JsonValue,
    stdlib_root: &Path,
    package_name: Option<&str>,
    expected_targets: &[&str],
) {
    assert_eq!(test_json["schema"], "ql.test.v1");
    assert_eq!(test_json["path"], json_path(stdlib_root));
    assert_eq!(test_json["requested_profile"], "debug");
    assert_eq!(test_json["profile_overridden"], false);
    match package_name {
        Some(package_name) => assert_eq!(test_json["package_name"], package_name),
        None => assert_eq!(test_json["package_name"], JsonValue::Null),
    }
    assert_eq!(test_json["filter"], JsonValue::Null);
    assert_eq!(test_json["list_only"], true);
    assert_eq!(test_json["status"], "listed");
    assert_eq!(
        test_json["discovered_total"],
        serde_json::json!(expected_targets.len())
    );
    assert_eq!(
        test_json["selected_total"],
        serde_json::json!(expected_targets.len())
    );
    assert_eq!(
        test_json["targets"],
        JsonValue::Array(
            expected_targets
                .iter()
                .map(|path| {
                    serde_json::json!({
                        "path": *path,
                        "kind": "smoke",
                        "profile": "debug",
                    })
                })
                .collect()
        ),
        "{context} should list the expected stdlib smoke targets"
    );
    assert_eq!(test_json["passed"], 0);
    assert_eq!(test_json["failed"], 0);
    assert_eq!(test_json["failures"], serde_json::json!([]));
}

pub fn assert_repo_stdlib_starter_dependencies_json(
    context: &str,
    dependencies_json: &JsonValue,
    stdlib_root: &Path,
) {
    assert_eq!(dependencies_json["schema"], "ql.project.dependencies.v1");
    assert_eq!(dependencies_json["path"], json_path(stdlib_root));
    assert_eq!(
        dependencies_json["workspace_manifest_path"],
        json_path(&stdlib_root.join("qlang.toml"))
    );
    assert_eq!(dependencies_json["package_name"], "stdlib.starter");

    let dependencies = dependencies_json["dependencies"]
        .as_array()
        .unwrap_or_else(|| panic!("{context} should expose dependencies: {dependencies_json}"));
    assert_eq!(
        dependencies.len(),
        5,
        "{context} should expose every stdlib starter dependency"
    );
    for (package_name, member, dependency_path) in [
        ("std.array", "packages/array", "../../packages/array"),
        ("std.core", "packages/core", "../../packages/core"),
        ("std.option", "packages/option", "../../packages/option"),
        ("std.result", "packages/result", "../../packages/result"),
        ("std.test", "packages/test", "../../packages/test"),
    ] {
        assert!(
            dependencies.iter().any(|actual| {
                actual["kind"] == "workspace"
                    && actual["package_name"] == package_name
                    && actual["member"] == member
                    && actual["dependency_path"] == dependency_path
                    && actual["manifest_path"]
                        == json_path(&stdlib_root.join(format!("{member}/qlang.toml")))
            }),
            "{context} should expose dependency `{package_name}`: {dependencies_json}"
        );
    }
}

pub fn assert_repo_stdlib_dependents_json(
    context: &str,
    dependents_json: &JsonValue,
    stdlib_root: &Path,
    package_name: &str,
    expected_dependents: &[(&str, &str)],
) {
    assert_eq!(dependents_json["schema"], "ql.project.dependents.v1");
    assert_eq!(dependents_json["path"], json_path(stdlib_root));
    assert_eq!(
        dependents_json["workspace_manifest_path"],
        json_path(&stdlib_root.join("qlang.toml"))
    );
    assert_eq!(dependents_json["package_name"], package_name);
    let dependents = dependents_json["dependents"]
        .as_array()
        .unwrap_or_else(|| panic!("{context} should expose dependents: {dependents_json}"));
    assert_eq!(
        dependents.len(),
        expected_dependents.len(),
        "{context} should expose expected dependent count for `{package_name}`"
    );

    for (dependent_name, member_path) in expected_dependents {
        assert!(
            dependents.iter().any(|actual| {
                actual["package_name"] == *dependent_name
                    && actual["member"] == *member_path
                    && actual["manifest_path"]
                        == json_path(&stdlib_root.join(format!("{member_path}/qlang.toml")))
            }),
            "{context} should expose dependent `{dependent_name}`: {dependents_json}"
        );
    }
}

pub fn write_repo_stdlib_fixture(temp: &TempDir, repo_root: &Path) -> PathBuf {
    let source_root = repo_root.join("stdlib");
    for relative in [
        "qlang.toml",
        "packages/core/qlang.toml",
        "packages/core/src/lib.ql",
        "packages/core/tests/smoke.ql",
        "packages/array/qlang.toml",
        "packages/array/src/lib.ql",
        "packages/array/tests/smoke.ql",
        "packages/option/qlang.toml",
        "packages/option/src/lib.ql",
        "packages/option/tests/smoke.ql",
        "packages/result/qlang.toml",
        "packages/result/src/lib.ql",
        "packages/result/tests/smoke.ql",
        "packages/test/qlang.toml",
        "packages/test/src/lib.ql",
        "packages/test/tests/smoke.ql",
        "examples/starter/qlang.toml",
        "examples/starter/src/lib.ql",
        "examples/starter/src/main.ql",
        "examples/starter/tests/smoke.ql",
    ] {
        let source_path = source_root.join(relative);
        let contents = fs::read_to_string(&source_path).unwrap_or_else(|error| {
            panic!("read stdlib fixture `{}`: {error}", source_path.display())
        });
        temp.write(&format!("stdlib/{relative}"), &contents);
    }
    temp.path().join("stdlib")
}
