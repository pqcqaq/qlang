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
