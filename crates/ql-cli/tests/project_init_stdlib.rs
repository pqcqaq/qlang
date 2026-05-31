mod support;

use std::path::{Path, PathBuf};

use serde_json::Value as JsonValue;
use support::project_init_stdlib::{
    assert_repo_stdlib_dependents_json, assert_repo_stdlib_starter_check_json,
    assert_repo_stdlib_starter_graph_json, assert_repo_stdlib_starter_status_json,
    assert_repo_stdlib_starter_targets_json, assert_repo_stdlib_test_list_json, json_path,
    parse_json_output, toolchain_available, write_repo_stdlib_fixture,
};
use support::{
    TempDir, executable_output_path, expect_empty_stderr, expect_exit_code, expect_file_exists,
    expect_silent_output, expect_stdout_contains_all, expect_success, ql_command,
    read_normalized_file, run_command_capture, static_library_output_path, workspace_root,
};

const STDLIB_PACKAGES: [(&str, &str); 5] = [
    ("std.array", "array"),
    ("std.core", "core"),
    ("std.option", "option"),
    ("std.result", "result"),
    ("std.test", "test"),
];

fn assert_stdlib_check_json(
    context: &str,
    check_json: &JsonValue,
    scope: &str,
    project_manifest: &Path,
    checked_files: &[PathBuf],
    stdlib_root: &Path,
) {
    assert_eq!(check_json["schema"], "ql.check.v1");
    assert_eq!(check_json["scope"], scope);
    assert_eq!(check_json["status"], "ok");
    assert_eq!(
        check_json["project_manifest_path"],
        json_path(project_manifest)
    );
    assert_eq!(check_json["diagnostic_files"], serde_json::json!([]));
    assert_eq!(check_json["failing_manifests"], serde_json::json!([]));
    assert_eq!(check_json["sync_interfaces"], false);
    assert_eq!(check_json["written_interfaces"], serde_json::json!([]));
    assert_eq!(
        check_json["checked_files"],
        serde_json::json!(
            checked_files
                .iter()
                .map(|path| json_path(path))
                .collect::<Vec<_>>()
        ),
        "{context} should report the initialized package sources"
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
        "{context} should load every initialized stdlib dependency interface"
    );
}

fn assert_repo_stdlib_targets_json(context: &str, targets_json: &JsonValue) {
    assert_eq!(targets_json["schema"], "ql.project.targets.v1");
    let members = targets_json["members"]
        .as_array()
        .unwrap_or_else(|| panic!("{context} should expose target members: {targets_json}"));
    assert_eq!(
        members.len(),
        6,
        "{context} should expose every stdlib target member"
    );

    for (package_name, manifest_path, expected_targets) in [
        (
            "std.core",
            "stdlib/packages/core/qlang.toml",
            vec![("lib", "src/lib.ql")],
        ),
        (
            "std.option",
            "stdlib/packages/option/qlang.toml",
            vec![("lib", "src/lib.ql")],
        ),
        (
            "std.result",
            "stdlib/packages/result/qlang.toml",
            vec![("lib", "src/lib.ql")],
        ),
        (
            "std.array",
            "stdlib/packages/array/qlang.toml",
            vec![("lib", "src/lib.ql")],
        ),
        (
            "std.test",
            "stdlib/packages/test/qlang.toml",
            vec![("lib", "src/lib.ql")],
        ),
        (
            "stdlib.starter",
            "stdlib/examples/starter/qlang.toml",
            vec![("lib", "src/lib.ql"), ("bin", "src/main.ql")],
        ),
    ] {
        let member = members
            .iter()
            .find(|actual| actual["package_name"] == package_name)
            .unwrap_or_else(|| panic!("{context} should expose targets for `{package_name}`"));
        assert_eq!(member["manifest_path"], manifest_path);
        let targets = member["targets"]
            .as_array()
            .unwrap_or_else(|| panic!("{context} should expose targets for `{package_name}`"));
        assert_eq!(
            targets.len(),
            expected_targets.len(),
            "{context} should expose expected target count for `{package_name}`"
        );
        for (kind, path) in expected_targets {
            assert!(
                targets
                    .iter()
                    .any(|actual| actual["kind"] == kind && actual["path"] == path),
                "{context} should expose `{kind}` target `{path}` for `{package_name}`: {targets_json}"
            );
        }
    }
}

fn assert_repo_stdlib_run_list_json(context: &str, targets_json: &JsonValue) {
    assert_eq!(targets_json["schema"], "ql.project.targets.v1");
    let members = targets_json["members"].as_array().unwrap_or_else(|| {
        panic!("{context} should expose runnable target members: {targets_json}")
    });
    assert_eq!(
        members.len(),
        6,
        "{context} should expose every stdlib workspace member"
    );

    for (package_name, manifest_path, expected_targets) in [
        (
            "std.core",
            "stdlib/packages/core/qlang.toml",
            Vec::<(&str, &str)>::new(),
        ),
        (
            "std.option",
            "stdlib/packages/option/qlang.toml",
            Vec::new(),
        ),
        (
            "std.result",
            "stdlib/packages/result/qlang.toml",
            Vec::new(),
        ),
        ("std.array", "stdlib/packages/array/qlang.toml", Vec::new()),
        ("std.test", "stdlib/packages/test/qlang.toml", Vec::new()),
        (
            "stdlib.starter",
            "stdlib/examples/starter/qlang.toml",
            vec![("bin", "src/main.ql")],
        ),
    ] {
        let member = members
            .iter()
            .find(|actual| actual["package_name"] == package_name)
            .unwrap_or_else(|| panic!("{context} should expose run-list member `{package_name}`"));
        assert_eq!(member["manifest_path"], manifest_path);
        let targets = member["targets"].as_array().unwrap_or_else(|| {
            panic!("{context} should expose runnable targets for `{package_name}`")
        });
        assert_eq!(
            targets.len(),
            expected_targets.len(),
            "{context} should expose runnable target count for `{package_name}`"
        );
        for (kind, path) in expected_targets {
            assert!(
                targets
                    .iter()
                    .any(|actual| actual["kind"] == kind && actual["path"] == path),
                "{context} should expose runnable `{kind}` target `{path}` for `{package_name}`: {targets_json}"
            );
        }
    }
}

fn repo_stdlib_artifact_path(package_dir: &str, kind: &str, stem: &str) -> String {
    repo_stdlib_artifact_path_for(Path::new("stdlib"), package_dir, kind, stem)
}

fn repo_stdlib_artifact_path_for(
    stdlib_root: &Path,
    package_dir: &str,
    kind: &str,
    stem: &str,
) -> String {
    let root = stdlib_root.join(package_dir).join("target/ql/debug");
    let path = match kind {
        "staticlib" => static_library_output_path(&root, stem),
        "exe" => executable_output_path(&root, stem),
        "llvm-ir" => root.join(format!("{stem}.ll")),
        _ => panic!("unsupported stdlib artifact kind `{kind}`"),
    };
    json_path(&path)
}

fn assert_repo_stdlib_build_json(context: &str, build_json: &JsonValue) {
    assert_eq!(build_json["schema"], "ql.build.v1");
    assert_eq!(build_json["scope"], "project");
    assert_eq!(build_json["path"], "stdlib");
    assert_eq!(build_json["project_manifest_path"], "stdlib/qlang.toml");
    assert_eq!(build_json["requested_emit"], "llvm-ir");
    assert_eq!(build_json["requested_profile"], "debug");
    assert_eq!(build_json["profile_overridden"], false);
    assert_eq!(build_json["emit_interface"], false);
    assert_eq!(build_json["status"], "ok");
    assert_eq!(build_json["failure"], JsonValue::Null);

    let interfaces = build_json["interfaces"]
        .as_array()
        .unwrap_or_else(|| panic!("{context} should expose interface writes: {build_json}"));
    assert_eq!(
        interfaces.len(),
        6,
        "{context} should rewrite every stdlib interface"
    );
    for (package_name, package_dir, interface_file) in [
        ("std.core", "packages/core", "std.core.qi"),
        ("std.option", "packages/option", "std.option.qi"),
        ("std.result", "packages/result", "std.result.qi"),
        ("std.array", "packages/array", "std.array.qi"),
        ("std.test", "packages/test", "std.test.qi"),
        ("stdlib.starter", "examples/starter", "stdlib.starter.qi"),
    ] {
        assert!(
            interfaces.iter().any(|actual| {
                actual["manifest_path"] == format!("stdlib/{package_dir}/qlang.toml")
                    && actual["package_name"] == package_name
                    && actual["path"] == format!("stdlib/{package_dir}/{interface_file}")
                    && actual["selected"] == true
                    && actual["status"] == "wrote"
            }),
            "{context} should report interface write for `{package_name}`: {build_json}"
        );
    }

    let built_targets = build_json["built_targets"]
        .as_array()
        .unwrap_or_else(|| panic!("{context} should expose built targets: {build_json}"));
    assert_eq!(
        built_targets.len(),
        7,
        "{context} should build every stdlib lib target and starter bin"
    );
    for (package_name, package_dir) in [
        ("std.core", "packages/core"),
        ("std.option", "packages/option"),
        ("std.result", "packages/result"),
        ("std.array", "packages/array"),
        ("std.test", "packages/test"),
        ("stdlib.starter", "examples/starter"),
    ] {
        assert!(
            built_targets.iter().any(|actual| {
                actual["manifest_path"] == format!("stdlib/{package_dir}/qlang.toml")
                    && actual["package_name"] == package_name
                    && actual["selected"] == true
                    && actual["dependency_only"] == false
                    && actual["kind"] == "lib"
                    && actual["path"] == "src/lib.ql"
                    && actual["emit"] == "staticlib"
                    && actual["profile"] == "debug"
                    && actual["artifact_path"]
                        == repo_stdlib_artifact_path(package_dir, "staticlib", "lib")
                    && actual["c_header_path"] == JsonValue::Null
            }),
            "{context} should include lib build target for `{package_name}`: {build_json}"
        );
    }
    assert!(
        built_targets.iter().any(|actual| {
            actual["manifest_path"] == "stdlib/examples/starter/qlang.toml"
                && actual["package_name"] == "stdlib.starter"
                && actual["selected"] == true
                && actual["dependency_only"] == false
                && actual["kind"] == "bin"
                && actual["path"] == "src/main.ql"
                && actual["emit"] == "llvm-ir"
                && actual["profile"] == "debug"
                && actual["artifact_path"]
                    == repo_stdlib_artifact_path("examples/starter", "llvm-ir", "main")
                && actual["c_header_path"] == JsonValue::Null
        }),
        "{context} should include starter bin llvm-ir target: {build_json}"
    );
}

fn assert_repo_stdlib_starter_build_json(
    context: &str,
    build_json: &JsonValue,
    stdlib_root: &Path,
) {
    assert_eq!(build_json["schema"], "ql.build.v1");
    assert_eq!(build_json["scope"], "project");
    assert_eq!(build_json["path"], json_path(stdlib_root));
    assert_eq!(
        build_json["project_manifest_path"],
        json_path(&stdlib_root.join("qlang.toml"))
    );
    assert_eq!(build_json["requested_emit"], "llvm-ir");
    assert_eq!(build_json["requested_profile"], "debug");
    assert_eq!(build_json["profile_overridden"], false);
    assert_eq!(build_json["emit_interface"], false);
    assert_eq!(build_json["status"], "ok");
    assert_eq!(build_json["failure"], JsonValue::Null);

    assert_eq!(
        build_json["interfaces"],
        serde_json::json!([
            {
                "manifest_path": json_path(&stdlib_root.join("examples/starter/qlang.toml")),
                "package_name": "stdlib.starter",
                "path": json_path(&stdlib_root.join("examples/starter/stdlib.starter.qi")),
                "selected": true,
                "status": "wrote",
            }
        ]),
        "{context} should write only the selected starter interface"
    );

    let built_targets = build_json["built_targets"]
        .as_array()
        .unwrap_or_else(|| panic!("{context} should expose built targets: {build_json}"));
    assert_eq!(
        built_targets.len(),
        7,
        "{context} should build the selected starter plus dependency closure"
    );
    for (package_name, package_dir) in [
        ("std.array", "packages/array"),
        ("std.core", "packages/core"),
        ("std.option", "packages/option"),
        ("std.result", "packages/result"),
        ("std.test", "packages/test"),
    ] {
        assert!(
            built_targets.iter().any(|actual| {
                actual["manifest_path"]
                    == json_path(&stdlib_root.join(format!("{package_dir}/qlang.toml")))
                    && actual["package_name"] == package_name
                    && actual["selected"] == false
                    && actual["dependency_only"] == true
                    && actual["kind"] == "lib"
                    && actual["path"] == "src/lib.ql"
                    && actual["emit"] == "staticlib"
                    && actual["profile"] == "debug"
                    && actual["artifact_path"]
                        == repo_stdlib_artifact_path_for(
                            stdlib_root,
                            package_dir,
                            "staticlib",
                            "lib",
                        )
                    && actual["c_header_path"] == JsonValue::Null
            }),
            "{context} should build dependency-only target for `{package_name}`: {build_json}"
        );
    }
    for (kind, path, emit, artifact_kind, stem) in [
        ("lib", "src/lib.ql", "staticlib", "staticlib", "lib"),
        ("bin", "src/main.ql", "llvm-ir", "llvm-ir", "main"),
    ] {
        assert!(
            built_targets.iter().any(|actual| {
                actual["manifest_path"]
                    == json_path(&stdlib_root.join("examples/starter/qlang.toml"))
                    && actual["package_name"] == "stdlib.starter"
                    && actual["selected"] == true
                    && actual["dependency_only"] == false
                    && actual["kind"] == kind
                    && actual["path"] == path
                    && actual["emit"] == emit
                    && actual["profile"] == "debug"
                    && actual["artifact_path"]
                        == repo_stdlib_artifact_path_for(
                            stdlib_root,
                            "examples/starter",
                            artifact_kind,
                            stem,
                        )
                    && actual["c_header_path"] == JsonValue::Null
            }),
            "{context} should build selected starter `{kind}` target: {build_json}"
        );
    }
}

fn assert_repo_stdlib_run_json(context: &str, run_json: &JsonValue, stdlib_root: &Path) {
    assert_eq!(run_json["schema"], "ql.run.v1");
    assert_eq!(run_json["scope"], "project");
    assert_eq!(run_json["path"], json_path(stdlib_root));
    assert_eq!(
        run_json["project_manifest_path"],
        json_path(&stdlib_root.join("qlang.toml"))
    );
    assert_eq!(run_json["requested_profile"], "debug");
    assert_eq!(run_json["profile_overridden"], false);
    assert_eq!(run_json["program_args"], serde_json::json!([]));
    assert_eq!(run_json["status"], "completed");
    assert_eq!(run_json["failure"], JsonValue::Null);
    assert_eq!(
        run_json["built_target"],
        serde_json::json!({
            "manifest_path": json_path(&stdlib_root.join("examples/starter/qlang.toml")),
            "package_name": "stdlib.starter",
            "selected": true,
            "dependency_only": false,
            "kind": "bin",
            "path": "src/main.ql",
            "emit": "exe",
            "profile": "debug",
            "artifact_path": repo_stdlib_artifact_path_for(
                stdlib_root,
                "examples/starter",
                "exe",
                "main"
            ),
            "c_header_path": JsonValue::Null,
        }),
        "{context} should run the stdlib starter executable"
    );
    assert_eq!(
        run_json["execution"],
        serde_json::json!({
            "exit_code": 0,
            "stdout": "",
            "stderr": "",
        })
    );
}

fn assert_stdlib_dependency_build_targets(context: &str, build_json: &JsonValue) {
    let built_targets = build_json["built_targets"]
        .as_array()
        .unwrap_or_else(|| panic!("{context} should expose built targets: {build_json}"));
    for package_name in [
        "std.array",
        "std.core",
        "std.option",
        "std.result",
        "std.test",
    ] {
        assert!(
            built_targets.iter().any(|target| {
                target["package_name"] == package_name
                    && target["dependency_only"] == true
                    && target["kind"] == "lib"
                    && target["selected"] == false
            }),
            "{context} should include dependency target `{package_name}`: {build_json}"
        );
    }
}

fn assert_build_json_includes_target(context: &str, build_json: &JsonValue, expected: JsonValue) {
    let built_targets = build_json["built_targets"]
        .as_array()
        .unwrap_or_else(|| panic!("{context} should expose built targets: {build_json}"));
    assert!(
        built_targets.iter().any(|target| target == &expected),
        "{context} should include target {expected}: {build_json}"
    );
}

fn assert_stdlib_workspace_graph_json(
    context: &str,
    graph_json: &JsonValue,
    member_manifest: &Path,
) {
    assert_stdlib_graph_json(
        context,
        graph_json,
        "app",
        member_manifest,
        "app.qi",
        "../../../stdlib/packages",
    );
}

fn assert_stdlib_graph_json(
    context: &str,
    graph_json: &JsonValue,
    package_name: &str,
    manifest_path: &Path,
    interface_path: &str,
    reference_prefix: &str,
) {
    assert_eq!(graph_json["schema"], "ql.project.graph.v1");
    assert_eq!(graph_json["package_name"], package_name);
    assert_eq!(graph_json["manifest_path"], json_path(manifest_path));
    assert_eq!(graph_json["interface"]["path"], interface_path);
    assert_eq!(graph_json["interface"]["status"], "valid");
    assert_eq!(graph_json["interface"]["detail"], JsonValue::Null);
    assert_eq!(
        graph_json["interface"]["stale_reasons"],
        serde_json::json!([])
    );
    assert_eq!(graph_json["workspace_members"], serde_json::json!([]));
    assert_eq!(graph_json["workspace_packages"], serde_json::json!([]));

    for (package_name, package_dir) in STDLIB_PACKAGES {
        let reference = format!("{reference_prefix}/{package_dir}");
        assert!(
            graph_json["references"]
                .as_array()
                .unwrap_or_else(|| panic!("{context} should expose references: {graph_json}"))
                .iter()
                .any(|actual| actual == reference.as_str()),
            "{context} should expose reference `{reference}`: {graph_json}"
        );
        assert!(
            graph_json["reference_interfaces"]
                .as_array()
                .unwrap_or_else(|| {
                    panic!("{context} should expose reference interfaces: {graph_json}")
                })
                .iter()
                .any(|actual| {
                    actual["package_name"] == package_name
                        && actual["reference"] == reference
                        && actual["status"] == "valid"
                        && actual["detail"] == JsonValue::Null
                        && actual["stale_reasons"] == serde_json::json!([])
                }),
            "{context} should expose valid interface for `{package_name}`: {graph_json}"
        );
    }
}

fn assert_stdlib_workspace_status_json(
    context: &str,
    status_json: &JsonValue,
    project_root: &Path,
    member_root: &Path,
) {
    assert_eq!(status_json["schema"], "ql.project.status.v1");
    assert_eq!(status_json["path"], json_path(project_root));
    assert_eq!(
        status_json["project_manifest_path"],
        json_path(&project_root.join("qlang.toml"))
    );
    assert_eq!(status_json["kind"], "workspace");
    assert_eq!(status_json["status"], "ok");
    let members = status_json["members"]
        .as_array()
        .unwrap_or_else(|| panic!("{context} should expose members: {status_json}"));
    assert_eq!(members.len(), 1, "{context} should select only app member");
    let member = &members[0];
    assert_eq!(member["member"], "packages/app");
    assert_eq!(member["package_name"], "app");
    assert_eq!(
        member["manifest_path"],
        json_path(&member_root.join("qlang.toml"))
    );
    assert_eq!(
        member["interface"]["path"],
        json_path(&member_root.join("app.qi"))
    );
    assert_eq!(member["interface"]["status"], "valid");
    assert_eq!(member["interface"]["detail"], JsonValue::Null);
    assert_eq!(member["interface"]["stale_reasons"], serde_json::json!([]));

    assert_stdlib_status_member_targets(context, member);
    assert_stdlib_status_member_dependencies(context, member, "../../../stdlib/packages");
}

fn assert_stdlib_package_status_json(context: &str, status_json: &JsonValue, project_root: &Path) {
    assert_eq!(status_json["schema"], "ql.project.status.v1");
    assert_eq!(status_json["path"], json_path(project_root));
    assert_eq!(
        status_json["project_manifest_path"],
        json_path(&project_root.join("qlang.toml"))
    );
    assert_eq!(status_json["kind"], "package");
    assert_eq!(status_json["status"], "ok");
    let members = status_json["members"]
        .as_array()
        .unwrap_or_else(|| panic!("{context} should expose members: {status_json}"));
    assert_eq!(members.len(), 1, "{context} should expose package member");
    let member = &members[0];
    assert_eq!(member["member"], JsonValue::Null);
    assert_eq!(member["package_name"], "demo-package");
    assert_eq!(
        member["manifest_path"],
        json_path(&project_root.join("qlang.toml"))
    );
    assert_eq!(
        member["interface"]["path"],
        json_path(&project_root.join("demo-package.qi"))
    );
    assert_eq!(member["interface"]["status"], "valid");
    assert_eq!(member["interface"]["detail"], JsonValue::Null);
    assert_eq!(member["interface"]["stale_reasons"], serde_json::json!([]));

    assert_stdlib_status_member_targets(context, member);
    assert_stdlib_status_member_dependencies(context, member, "../stdlib/packages");
}

fn assert_stdlib_dependencies_json(
    context: &str,
    dependencies_json: &JsonValue,
    request_path: &Path,
    manifest_path: &Path,
    package_name: &str,
    dependency_prefix: &str,
    stdlib_root: &Path,
) {
    assert_eq!(dependencies_json["schema"], "ql.project.dependencies.v1");
    assert_eq!(dependencies_json["path"], json_path(request_path));
    assert_eq!(
        dependencies_json["workspace_manifest_path"],
        json_path(manifest_path)
    );
    assert_eq!(dependencies_json["package_name"], package_name);
    let dependencies = dependencies_json["dependencies"]
        .as_array()
        .unwrap_or_else(|| panic!("{context} should expose dependencies: {dependencies_json}"));
    assert_eq!(
        dependencies.len(),
        STDLIB_PACKAGES.len(),
        "{context} should expose every initialized stdlib dependency: {dependencies_json}"
    );

    for (package_name, package_dir) in STDLIB_PACKAGES {
        let dependency_path = format!("{dependency_prefix}/{package_dir}");
        let dependency_manifest = stdlib_root
            .join("packages")
            .join(package_dir)
            .join("qlang.toml");
        assert!(
            dependencies.iter().any(|actual| {
                actual["kind"] == "local"
                    && actual["member"] == JsonValue::Null
                    && actual["package_name"] == package_name
                    && actual["dependency_path"] == dependency_path
                    && actual["manifest_path"] == json_path(&dependency_manifest)
            }),
            "{context} should expose stdlib dependency `{package_name}`: {dependencies_json}"
        );
    }
}

fn assert_stdlib_status_member_targets(context: &str, member: &JsonValue) {
    for (kind, path) in [("lib", "src/lib.ql"), ("bin", "src/main.ql")] {
        assert!(
            member["targets"]
                .as_array()
                .unwrap_or_else(|| panic!("{context} should expose targets: {member}"))
                .iter()
                .any(|actual| actual["kind"] == kind && actual["path"] == path),
            "{context} should expose `{kind}` target `{path}`: {member}"
        );
    }
}

fn assert_stdlib_status_member_dependencies(
    context: &str,
    member: &JsonValue,
    dependency_prefix: &str,
) {
    for (package_name, package_dir) in STDLIB_PACKAGES {
        let dependency_path = format!("{dependency_prefix}/{package_dir}");
        assert!(
            member["dependencies"]
                .as_array()
                .unwrap_or_else(|| panic!("{context} should expose dependencies: {member}"))
                .iter()
                .any(|actual| {
                    actual["kind"] == "local"
                        && actual["package_name"] == package_name
                        && actual["dependency_path"] == dependency_path
                }),
            "{context} should expose local dependency `{package_name}`: {member}"
        );
    }
}

fn expect_stdlib_starter_source(source: &str, context: &str) {
    for needle in [
        "use std.array.repeat_array as repeat_array",
        "use std.option.Option as Option",
        "use std.result.error_to_option as result_error_to_option",
        "use std.result.ok_or as result_ok_or",
        "use std.result.to_option as result_to_option",
        "let repeated: [Int; 3] = repeat_array(1)",
        "let failed: Result[Int, Int] = result_ok_or(missing, 7)",
    ] {
        assert!(
            source.contains(needle),
            "{context} should contain `{needle}`\n{source}"
        );
    }
    for legacy in [
        "repeat3_array",
        "reverse3_array",
        "some_int",
        "ok_int",
        "unwrap_result_or as result_unwrap_result_or",
    ] {
        assert!(
            !source.contains(legacy),
            "{context} should not contain legacy API `{legacy}`\n{source}"
        );
    }
}

fn expect_stdlib_starter_main_source(source: &str, context: &str) {
    for needle in [
        "use std.array.repeat_array as repeat_array",
        "use std.result.to_option as result_to_option",
        "let repeated_false: [Bool; 3] = repeat_array(false)",
        "let repeated_enabled: [Bool; 3] = [option_unwrap_or(enabled, false); 3]",
    ] {
        assert!(
            source.contains(needle),
            "{context} should contain `{needle}`\n{source}"
        );
    }
    for legacy in ["repeat3_array", "reverse3_array", "some_bool", "ok_bool"] {
        assert!(
            !source.contains(legacy),
            "{context} should not contain legacy API `{legacy}`\n{source}"
        );
    }
}

fn expect_stdlib_starter_smoke_source(source: &str, context: &str) {
    for needle in [
        "use std.array.repeat_array as repeat_array",
        "use std.result.ok_or as result_ok_or",
        "use std.result.to_option as result_to_option",
        "use std.test.expect_array_eq as expect_array_eq",
        "use std.test.expect_array_reverse as expect_array_reverse",
        "use std.test.expect_eq as expect_eq",
        "use std.test.expect_option_none as expect_option_none",
        "use std.test.expect_option_some as expect_option_some",
        "use std.test.expect_result_err as expect_result_err",
        "use std.test.expect_result_ok as expect_result_ok",
        "let repeated: [Int; 3] = repeat_array(2)",
        "let array_check = expect_array_eq(repeated, [2, 2, 2]) + expect_array_reverse(numbers, [3, 2, 1])",
        "let result_value: Result[Int, Int] = result_ok_or(option_value, 9)",
        "let failed: Result[Int, Int] = result_ok_or(missing, 4)",
        "let option_check = expect_option_some(option_value, 6) + expect_option_none(missing)",
        "let result_check = expect_result_ok(result_value, 6) + expect_result_err(failed, 4)",
        "return expect_eq(total_check + length_check + contains_check + repeated_check + array_check + option_check + result_check, 0)",
    ] {
        assert!(
            source.contains(needle),
            "{context} should contain `{needle}`\n{source}"
        );
    }
    assert!(
        !source.contains("result_error_to_option"),
        "{context} should not use conversion-only result assertions\n{source}"
    );
    for legacy in [
        "repeat3_array",
        "reverse3_array",
        "some_int",
        "ok_int",
        "expect_status_ok",
    ] {
        assert!(
            !source.contains(legacy),
            "{context} should not contain legacy API `{legacy}`\n{source}"
        );
    }
}

fn expect_stdlib_starter_interface(source: &str, package_name: &str, context: &str) {
    for needle in &[
        "// qlang interface v1".to_owned(),
        format!("// package: {package_name}"),
        "// source: src/lib.ql".to_owned(),
        "use std.array.repeat_array as repeat_array".to_owned(),
        "use std.option.Option as Option".to_owned(),
        "use std.result.Result as Result".to_owned(),
        "use std.result.ok_or as result_ok_or".to_owned(),
        "pub fn run() -> Int".to_owned(),
    ] {
        assert!(
            source.contains(needle),
            "{context} should contain `{needle}`\n{source}"
        );
    }
    for legacy in ["repeat3_array", "reverse3_array", "some_int", "ok_int"] {
        assert!(
            !source.contains(legacy),
            "{context} should not contain legacy API `{legacy}`\n{source}"
        );
    }
}

fn expect_emit_interface_check_ok(
    case_name: &str,
    workspace_root: &Path,
    project_root: &Path,
    package_name: Option<&str>,
    interface_path: &Path,
    description: &str,
) {
    let mut command = ql_command(workspace_root);
    command
        .args(["project", "emit-interface", "--check"])
        .arg(project_root);
    if let Some(package_name) = package_name {
        command.args(["--package", package_name]);
    }
    let output = run_command_capture(&mut command, description);
    let (stdout, stderr) = expect_success(
        case_name,
        "emit interface check initialized scaffold",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        case_name,
        "emit interface check initialized scaffold",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        case_name,
        &stdout.replace('\\', "/"),
        &[&format!(
            "ok interface: {}",
            interface_path.display().to_string().replace('\\', "/")
        )],
    )
    .unwrap();
}

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
    let expected = serde_json::json!({
        "schema": "ql.test.v1",
        "path": json_path(&stdlib_root),
        "requested_profile": "debug",
        "profile_overridden": false,
        "package_name": "stdlib.starter",
        "filter": JsonValue::Null,
        "list_only": false,
        "status": "ok",
        "discovered_total": 1,
        "selected_total": 1,
        "targets": [
            {
                "path": "examples/starter/tests/smoke.ql",
                "kind": "smoke",
                "profile": "debug",
            },
        ],
        "passed": 1,
        "failed": 0,
        "failures": [],
    });
    assert_eq!(
        actual, expected,
        "copied repo stdlib starter package should keep a stable test json contract"
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

#[test]
fn repo_stdlib_workspace_starter_metadata_selectors_are_current() {
    let workspace_root = workspace_root();

    let mut graph = ql_command(&workspace_root);
    graph.args([
        "project",
        "graph",
        "stdlib",
        "--package",
        "stdlib.starter",
        "--json",
    ]);
    let output = run_command_capture(
        &mut graph,
        "`ql project graph stdlib --package stdlib.starter --json`",
    );
    let (stdout, stderr) = expect_success(
        "repo-stdlib-workspace-starter-metadata",
        "graph repo stdlib starter package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "repo-stdlib-workspace-starter-metadata",
        "graph repo stdlib starter package",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("repo-stdlib-workspace-starter-metadata", &stdout);
    assert_repo_stdlib_starter_graph_json(
        "repo stdlib starter graph json",
        &actual,
        Path::new("stdlib"),
    );

    let mut status = ql_command(&workspace_root);
    status.args([
        "project",
        "status",
        "stdlib",
        "--package",
        "stdlib.starter",
        "--json",
    ]);
    let output = run_command_capture(
        &mut status,
        "`ql project status stdlib --package stdlib.starter --json`",
    );
    let (stdout, stderr) = expect_success(
        "repo-stdlib-workspace-starter-metadata",
        "status repo stdlib starter package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "repo-stdlib-workspace-starter-metadata",
        "status repo stdlib starter package",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("repo-stdlib-workspace-starter-metadata", &stdout);
    assert_repo_stdlib_starter_status_json(
        "repo stdlib starter status json",
        &actual,
        Path::new("stdlib"),
    );

    let mut targets = ql_command(&workspace_root);
    targets.args([
        "project",
        "targets",
        "stdlib",
        "--package",
        "stdlib.starter",
        "--json",
    ]);
    let output = run_command_capture(
        &mut targets,
        "`ql project targets stdlib --package stdlib.starter --json`",
    );
    let (stdout, stderr) = expect_success(
        "repo-stdlib-workspace-starter-metadata",
        "targets repo stdlib starter package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "repo-stdlib-workspace-starter-metadata",
        "targets repo stdlib starter package",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("repo-stdlib-workspace-starter-metadata", &stdout);
    assert_repo_stdlib_starter_targets_json(
        "repo stdlib starter targets json",
        &actual,
        Path::new("stdlib"),
    );
}

#[test]
fn repo_stdlib_workspace_targets_and_dependents_are_current() {
    let workspace_root = workspace_root();

    let mut targets = ql_command(&workspace_root);
    targets.args(["project", "targets", "stdlib", "--json"]);
    let output = run_command_capture(&mut targets, "`ql project targets stdlib --json`");
    let (stdout, stderr) = expect_success(
        "repo-stdlib-workspace-targets",
        "targets repo stdlib workspace",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "repo-stdlib-workspace-targets",
        "targets repo stdlib workspace",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("repo-stdlib-workspace-targets", &stdout);
    assert_repo_stdlib_targets_json("repo stdlib workspace targets json", &actual);

    let mut option_dependents = ql_command(&workspace_root);
    option_dependents.args([
        "project",
        "dependents",
        "stdlib",
        "--name",
        "std.option",
        "--json",
    ]);
    let output = run_command_capture(
        &mut option_dependents,
        "`ql project dependents stdlib --name std.option --json`",
    );
    let (stdout, stderr) = expect_success(
        "repo-stdlib-workspace-targets",
        "std.option dependents in repo stdlib workspace",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "repo-stdlib-workspace-targets",
        "std.option dependents in repo stdlib workspace",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("repo-stdlib-workspace-targets", &stdout);
    assert_repo_stdlib_dependents_json(
        "repo stdlib std.option dependents json",
        &actual,
        Path::new("stdlib"),
        "std.option",
        &[
            ("std.result", "packages/result"),
            ("std.test", "packages/test"),
            ("stdlib.starter", "examples/starter"),
        ],
    );

    let mut option_package_dependents = ql_command(&workspace_root);
    option_package_dependents.args([
        "project",
        "dependents",
        "stdlib",
        "--package",
        "std.option",
        "--json",
    ]);
    let output = run_command_capture(
        &mut option_package_dependents,
        "`ql project dependents stdlib --package std.option --json`",
    );
    let (stdout, stderr) = expect_success(
        "repo-stdlib-workspace-targets",
        "std.option dependents in repo stdlib workspace by package selector",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "repo-stdlib-workspace-targets",
        "std.option dependents in repo stdlib workspace by package selector",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("repo-stdlib-workspace-targets", &stdout);
    assert_repo_stdlib_dependents_json(
        "repo stdlib std.option dependents package selector json",
        &actual,
        Path::new("stdlib"),
        "std.option",
        &[
            ("std.result", "packages/result"),
            ("std.test", "packages/test"),
            ("stdlib.starter", "examples/starter"),
        ],
    );

    let mut core_dependents = ql_command(&workspace_root);
    core_dependents.args([
        "project",
        "dependents",
        "stdlib",
        "--name",
        "std.core",
        "--json",
    ]);
    let output = run_command_capture(
        &mut core_dependents,
        "`ql project dependents stdlib --name std.core --json`",
    );
    let (stdout, stderr) = expect_success(
        "repo-stdlib-workspace-targets",
        "std.core dependents in repo stdlib workspace",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "repo-stdlib-workspace-targets",
        "std.core dependents in repo stdlib workspace",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("repo-stdlib-workspace-targets", &stdout);
    assert_repo_stdlib_dependents_json(
        "repo stdlib std.core dependents json",
        &actual,
        Path::new("stdlib"),
        "std.core",
        &[
            ("std.array", "packages/array"),
            ("std.test", "packages/test"),
            ("stdlib.starter", "examples/starter"),
        ],
    );
}

#[test]
fn repo_stdlib_workspace_lists_build_run_and_tests() {
    let workspace_root = workspace_root();

    let mut build_list = ql_command(&workspace_root);
    build_list.args(["build", "stdlib", "--list", "--json"]);
    let output = run_command_capture(&mut build_list, "`ql build stdlib --list --json`");
    let (stdout, stderr) = expect_success(
        "repo-stdlib-workspace-list",
        "list build targets in repo stdlib workspace",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "repo-stdlib-workspace-list",
        "list build targets in repo stdlib workspace",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("repo-stdlib-workspace-list", &stdout);
    assert_repo_stdlib_targets_json("repo stdlib build list json", &actual);

    let mut run_list = ql_command(&workspace_root);
    run_list.args(["run", "stdlib", "--list", "--json"]);
    let output = run_command_capture(&mut run_list, "`ql run stdlib --list --json`");
    let (stdout, stderr) = expect_success(
        "repo-stdlib-workspace-list",
        "list runnable targets in repo stdlib workspace",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "repo-stdlib-workspace-list",
        "list runnable targets in repo stdlib workspace",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("repo-stdlib-workspace-list", &stdout);
    assert_repo_stdlib_run_list_json("repo stdlib run list json", &actual);

    let all_smoke_targets = [
        "packages/core/tests/smoke.ql",
        "packages/option/tests/smoke.ql",
        "packages/result/tests/smoke.ql",
        "packages/array/tests/smoke.ql",
        "packages/test/tests/smoke.ql",
        "examples/starter/tests/smoke.ql",
    ];

    let mut test_list = ql_command(&workspace_root);
    test_list.args(["test", "stdlib", "--list", "--json"]);
    let output = run_command_capture(&mut test_list, "`ql test stdlib --list --json`");
    let (stdout, stderr) = expect_success(
        "repo-stdlib-workspace-list",
        "list all smoke tests in repo stdlib workspace",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "repo-stdlib-workspace-list",
        "list all smoke tests in repo stdlib workspace",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("repo-stdlib-workspace-list", &stdout);
    assert_repo_stdlib_test_list_json(
        "repo stdlib test list json",
        &actual,
        Path::new("stdlib"),
        None,
        &all_smoke_targets,
    );

    let mut starter_test_list = ql_command(&workspace_root);
    starter_test_list.args([
        "test",
        "stdlib",
        "--list",
        "--json",
        "--package",
        "stdlib.starter",
    ]);
    let output = run_command_capture(
        &mut starter_test_list,
        "`ql test stdlib --list --json --package stdlib.starter`",
    );
    let (stdout, stderr) = expect_success(
        "repo-stdlib-workspace-list",
        "list starter smoke tests in repo stdlib workspace",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "repo-stdlib-workspace-list",
        "list starter smoke tests in repo stdlib workspace",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("repo-stdlib-workspace-list", &stdout);
    assert_repo_stdlib_test_list_json(
        "repo stdlib starter test list json",
        &actual,
        Path::new("stdlib"),
        Some("stdlib.starter"),
        &["examples/starter/tests/smoke.ql"],
    );
}

#[test]
fn repo_stdlib_workspace_checks_builds_and_tests_starter_package() {
    if !toolchain_available("`ql check/build/test --package` repo stdlib starter") {
        return;
    }

    let workspace_root = workspace_root();

    let mut check = ql_command(&workspace_root);
    check.args(["check", "stdlib", "--package", "stdlib.starter", "--json"]);
    let output = run_command_capture(
        &mut check,
        "`ql check stdlib --package stdlib.starter --json`",
    );
    let (stdout, stderr) = expect_success(
        "repo-stdlib-workspace-starter-package",
        "check repo stdlib starter package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "repo-stdlib-workspace-starter-package",
        "check repo stdlib starter package",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("repo-stdlib-workspace-starter-package", &stdout);
    assert_repo_stdlib_starter_check_json(
        "repo stdlib starter check json",
        &actual,
        Path::new("stdlib"),
    );

    let mut build = ql_command(&workspace_root);
    build.args(["build", "stdlib", "--package", "stdlib.starter", "--json"]);
    let output = run_command_capture(
        &mut build,
        "`ql build stdlib --package stdlib.starter --json`",
    );
    let (stdout, stderr) = expect_success(
        "repo-stdlib-workspace-starter-package",
        "build repo stdlib starter package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "repo-stdlib-workspace-starter-package",
        "build repo stdlib starter package",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("repo-stdlib-workspace-starter-package", &stdout);
    assert_repo_stdlib_starter_build_json(
        "repo stdlib starter build json",
        &actual,
        Path::new("stdlib"),
    );

    let mut test = ql_command(&workspace_root);
    test.args(["test", "stdlib", "--package", "stdlib.starter", "--json"]);
    let output = run_command_capture(
        &mut test,
        "`ql test stdlib --package stdlib.starter --json`",
    );
    let (stdout, stderr) = expect_success(
        "repo-stdlib-workspace-starter-package",
        "test repo stdlib starter package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "repo-stdlib-workspace-starter-package",
        "test repo stdlib starter package",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("repo-stdlib-workspace-starter-package", &stdout);
    let expected = serde_json::json!({
        "schema": "ql.test.v1",
        "path": "stdlib",
        "requested_profile": "debug",
        "profile_overridden": false,
        "package_name": "stdlib.starter",
        "filter": JsonValue::Null,
        "list_only": false,
        "status": "ok",
        "discovered_total": 1,
        "selected_total": 1,
        "targets": [
            {
                "path": "examples/starter/tests/smoke.ql",
                "kind": "smoke",
                "profile": "debug",
            },
        ],
        "passed": 1,
        "failed": 0,
        "failures": [],
    });
    assert_eq!(
        actual, expected,
        "repo stdlib starter package should keep a stable test json contract"
    );
}

#[test]
fn repo_stdlib_workspace_builds_and_runs_starter() {
    if !toolchain_available("`ql build/run` repo stdlib workspace") {
        return;
    }

    let workspace_root = workspace_root();

    let mut build = ql_command(&workspace_root);
    build.args(["build", "stdlib", "--json"]);
    let output = run_command_capture(&mut build, "`ql build stdlib --json`");
    let (stdout, stderr) = expect_success(
        "repo-stdlib-workspace-build-run",
        "build repo stdlib workspace",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "repo-stdlib-workspace-build-run",
        "build repo stdlib workspace",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("repo-stdlib-workspace-build-run", &stdout);
    assert_repo_stdlib_build_json("repo stdlib workspace build json", &actual);
    for (context, path) in [
        (
            "std.core static library",
            static_library_output_path(
                &workspace_root.join("stdlib/packages/core/target/ql/debug"),
                "lib",
            ),
        ),
        (
            "std.option static library",
            static_library_output_path(
                &workspace_root.join("stdlib/packages/option/target/ql/debug"),
                "lib",
            ),
        ),
        (
            "std.result static library",
            static_library_output_path(
                &workspace_root.join("stdlib/packages/result/target/ql/debug"),
                "lib",
            ),
        ),
        (
            "std.array static library",
            static_library_output_path(
                &workspace_root.join("stdlib/packages/array/target/ql/debug"),
                "lib",
            ),
        ),
        (
            "std.test static library",
            static_library_output_path(
                &workspace_root.join("stdlib/packages/test/target/ql/debug"),
                "lib",
            ),
        ),
        (
            "stdlib starter static library",
            static_library_output_path(
                &workspace_root.join("stdlib/examples/starter/target/ql/debug"),
                "lib",
            ),
        ),
        (
            "stdlib starter llvm-ir",
            workspace_root.join("stdlib/examples/starter/target/ql/debug/main.ll"),
        ),
    ] {
        expect_file_exists(
            "repo-stdlib-workspace-build-run",
            &path,
            context,
            "`ql build stdlib --json`",
        )
        .unwrap();
    }

    let mut run = ql_command(&workspace_root);
    run.args(["run", "stdlib", "--package", "stdlib.starter", "--json"]);
    let output = run_command_capture(&mut run, "`ql run stdlib --package stdlib.starter --json`");
    let (stdout, stderr) = expect_success(
        "repo-stdlib-workspace-build-run",
        "run repo stdlib starter",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "repo-stdlib-workspace-build-run",
        "run repo stdlib starter",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("repo-stdlib-workspace-build-run", &stdout);
    assert_repo_stdlib_run_json("repo stdlib starter run json", &actual, Path::new("stdlib"));
    expect_file_exists(
        "repo-stdlib-workspace-build-run",
        &executable_output_path(
            &workspace_root.join("stdlib/examples/starter/target/ql/debug"),
            "main",
        ),
        "stdlib starter executable",
        "`ql run stdlib --package stdlib.starter --json`",
    )
    .unwrap();
}

#[test]
fn project_init_with_stdlib_creates_consuming_package_scaffold_and_check_succeeds() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-init-stdlib-package");
    let stdlib_root = write_repo_stdlib_fixture(&temp, &workspace_root);
    let project_root = temp.path().join("demo-package");

    let mut init = ql_command(&workspace_root);
    init.args([
        "project",
        "init",
        &project_root.to_string_lossy(),
        "--stdlib",
        &stdlib_root.to_string_lossy(),
    ]);
    let output = run_command_capture(&mut init, "`ql project init --stdlib` package");
    let (_stdout, stderr) = expect_success(
        "project-init-stdlib-package",
        "stdlib package init",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-package",
        "stdlib package init",
        &stderr,
    )
    .unwrap();

    assert_eq!(
        read_normalized_file(&project_root.join("qlang.toml"), "stdlib package manifest"),
        "[package]\nname = \"demo-package\"\n\n[dependencies]\n\"std.core\" = \"../stdlib/packages/core\"\n\"std.option\" = \"../stdlib/packages/option\"\n\"std.result\" = \"../stdlib/packages/result\"\n\"std.array\" = \"../stdlib/packages/array\"\n\"std.test\" = \"../stdlib/packages/test\"\n"
    );
    let lib_source =
        read_normalized_file(&project_root.join("src/lib.ql"), "stdlib package source");
    expect_stdlib_starter_source(&lib_source, "stdlib package source");
    let main_source = read_normalized_file(
        &project_root.join("src/main.ql"),
        "stdlib package main source",
    );
    expect_stdlib_starter_main_source(&main_source, "stdlib package main source");
    let smoke_source = read_normalized_file(
        &project_root.join("tests/smoke.ql"),
        "stdlib package smoke test",
    );
    expect_stdlib_starter_smoke_source(&smoke_source, "stdlib package smoke test");

    let mut check = ql_command(&workspace_root);
    check.args(["check", &project_root.to_string_lossy()]);
    let output = run_command_capture(&mut check, "`ql check` initialized stdlib package");
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-package",
        "check initialized stdlib package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-package",
        "check initialized stdlib package",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "project-init-stdlib-package",
        &stdout.replace('\\', "/"),
        &[
            &format!(
                "ok: {}",
                project_root
                    .join("src")
                    .join("lib.ql")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            "loaded interface:",
        ],
    )
    .unwrap();

    let mut check_json = ql_command(&workspace_root);
    check_json.args(["check", &project_root.to_string_lossy(), "--json"]);
    let output = run_command_capture(
        &mut check_json,
        "`ql check --json` initialized stdlib package",
    );
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-package",
        "json check initialized stdlib package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-package",
        "json check initialized stdlib package",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("project-init-stdlib-package", &stdout);
    assert_stdlib_check_json(
        "initialized stdlib package check json",
        &actual,
        "package",
        &project_root.join("qlang.toml"),
        &[
            project_root.join("src/lib.ql"),
            project_root.join("src/main.ql"),
        ],
        &stdlib_root,
    );

    let package_interface = project_root.join("demo-package.qi");
    let mut emit_interface = ql_command(&workspace_root);
    emit_interface.args(["project", "emit-interface", &project_root.to_string_lossy()]);
    let output = run_command_capture(
        &mut emit_interface,
        "`ql project emit-interface` initialized stdlib package",
    );
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-package",
        "emit interface initialized stdlib package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-package",
        "emit interface initialized stdlib package",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "project-init-stdlib-package",
        &stdout.replace('\\', "/"),
        &[&format!(
            "wrote interface: {}",
            package_interface.display().to_string().replace('\\', "/")
        )],
    )
    .unwrap();
    let interface_source = read_normalized_file(
        &package_interface,
        "initialized stdlib package interface artifact",
    );
    expect_stdlib_starter_interface(
        &interface_source,
        "demo-package",
        "initialized stdlib package interface artifact",
    );
    expect_emit_interface_check_ok(
        "project-init-stdlib-package",
        &workspace_root,
        &project_root,
        None,
        &package_interface,
        "`ql project emit-interface --check` initialized stdlib package",
    );

    let mut graph_json = ql_command(&workspace_root);
    graph_json.args([
        "project",
        "graph",
        &project_root.to_string_lossy(),
        "--json",
    ]);
    let output = run_command_capture(
        &mut graph_json,
        "`ql project graph --json` initialized stdlib package",
    );
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-package",
        "graph json initialized stdlib package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-package",
        "graph json initialized stdlib package",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("project-init-stdlib-package", &stdout);
    assert_stdlib_graph_json(
        "initialized stdlib package graph json",
        &actual,
        "demo-package",
        &project_root.join("qlang.toml"),
        "demo-package.qi",
        "../stdlib/packages",
    );

    let mut status_json = ql_command(&workspace_root);
    status_json.args([
        "project",
        "status",
        &project_root.to_string_lossy(),
        "--json",
    ]);
    let output = run_command_capture(
        &mut status_json,
        "`ql project status --json` initialized stdlib package",
    );
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-package",
        "status json initialized stdlib package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-package",
        "status json initialized stdlib package",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("project-init-stdlib-package", &stdout);
    assert_stdlib_package_status_json(
        "initialized stdlib package status json",
        &actual,
        &project_root,
    );

    let mut dependencies = ql_command(&workspace_root);
    dependencies.args(["project", "dependencies", &project_root.to_string_lossy()]);
    let output = run_command_capture(
        &mut dependencies,
        "`ql project dependencies` initialized stdlib package",
    );
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-package",
        "dependencies initialized stdlib package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-package",
        "dependencies initialized stdlib package",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "project-init-stdlib-package",
        &stdout.replace('\\', "/"),
        &[
            &format!(
                "workspace_manifest: {}",
                project_root
                    .join("qlang.toml")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            "package: demo-package",
            "dependencies:",
            "  - ../stdlib/packages/core (std.core, local)",
            "  - ../stdlib/packages/option (std.option, local)",
            "  - ../stdlib/packages/result (std.result, local)",
            "  - ../stdlib/packages/array (std.array, local)",
            "  - ../stdlib/packages/test (std.test, local)",
        ],
    )
    .unwrap();

    let mut dependencies_json = ql_command(&workspace_root);
    dependencies_json.args([
        "project",
        "dependencies",
        &project_root.to_string_lossy(),
        "--json",
    ]);
    let output = run_command_capture(
        &mut dependencies_json,
        "`ql project dependencies --json` initialized stdlib package",
    );
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-package",
        "dependencies json initialized stdlib package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-package",
        "dependencies json initialized stdlib package",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("project-init-stdlib-package", &stdout);
    assert_stdlib_dependencies_json(
        "initialized stdlib package dependencies json",
        &actual,
        &project_root,
        &project_root.join("qlang.toml"),
        "demo-package",
        "../stdlib/packages",
        &stdlib_root,
    );

    let mut dependencies_selector_json = ql_command(&workspace_root);
    dependencies_selector_json.args([
        "project",
        "dependencies",
        &project_root.to_string_lossy(),
        "--name",
        "demo-package",
        "--json",
    ]);
    let output = run_command_capture(
        &mut dependencies_selector_json,
        "`ql project dependencies --json --name demo-package` initialized stdlib package",
    );
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-package",
        "dependencies json selector initialized stdlib package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-package",
        "dependencies json selector initialized stdlib package",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("project-init-stdlib-package", &stdout);
    assert_stdlib_dependencies_json(
        "initialized stdlib package dependencies selector json",
        &actual,
        &project_root,
        &project_root.join("qlang.toml"),
        "demo-package",
        "../stdlib/packages",
        &stdlib_root,
    );

    let mut dependents_selector_json = ql_command(&workspace_root);
    dependents_selector_json.args([
        "project",
        "dependents",
        &project_root.to_string_lossy(),
        "--name",
        "demo-package",
        "--json",
    ]);
    let output = run_command_capture(
        &mut dependents_selector_json,
        "`ql project dependents --json --name demo-package` initialized stdlib package",
    );
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-package",
        "dependents json selector initialized stdlib package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-package",
        "dependents json selector initialized stdlib package",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("project-init-stdlib-package", &stdout);
    assert_eq!(actual["schema"], "ql.project.dependents.v1");
    assert_eq!(actual["path"], json_path(&project_root));
    assert_eq!(
        actual["workspace_manifest_path"],
        json_path(&project_root.join("qlang.toml"))
    );
    assert_eq!(actual["package_name"], "demo-package");
    assert_eq!(actual["dependents"], serde_json::json!([]));
}

#[test]
fn project_init_with_stdlib_creates_runnable_and_testable_package_scaffold() {
    if !toolchain_available("`ql project init --stdlib` runnable package test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-init-stdlib-package-run");
    let stdlib_root = write_repo_stdlib_fixture(&temp, &workspace_root);
    let project_root = temp.path().join("demo-package");
    let package_manifest = project_root.join("qlang.toml");
    let package_library_output =
        static_library_output_path(&project_root.join("target/ql/debug"), "lib");
    let package_build_output = project_root.join("target/ql/debug/main.ll");
    let package_run_output = executable_output_path(&project_root.join("target/ql/debug"), "main");
    let package_interface_output = project_root.join("demo-package.qi");

    let mut init = ql_command(&workspace_root);
    init.args([
        "project",
        "init",
        &project_root.to_string_lossy(),
        "--stdlib",
        &stdlib_root.to_string_lossy(),
    ]);
    let output = run_command_capture(&mut init, "`ql project init --stdlib` runnable package");
    let (_stdout, stderr) = expect_success(
        "project-init-stdlib-package-run",
        "stdlib package init for runnable scaffold",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-package-run",
        "stdlib package init for runnable scaffold",
        &stderr,
    )
    .unwrap();

    let mut build_json = ql_command(&workspace_root);
    build_json.current_dir(temp.path());
    build_json.args(["build"]).arg(&project_root).arg("--json");
    let output = run_command_capture(
        &mut build_json,
        "`ql build --json` initialized stdlib package",
    );
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-package-run",
        "json build initialized stdlib package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-package-run",
        "json build initialized stdlib package",
        &stderr,
    )
    .unwrap();

    let build_json = parse_json_output("project-init-stdlib-package-run", &stdout);
    assert_eq!(build_json["schema"], "ql.build.v1");
    assert_eq!(
        build_json["path"],
        project_root.display().to_string().replace('\\', "/")
    );
    assert_eq!(build_json["scope"], "project");
    assert_eq!(
        build_json["project_manifest_path"],
        package_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(build_json["requested_emit"], "llvm-ir");
    assert_eq!(build_json["requested_profile"], "debug");
    assert_eq!(build_json["profile_overridden"], false);
    assert_eq!(build_json["emit_interface"], false);
    assert_eq!(build_json["status"], "ok");
    assert_eq!(build_json["failure"], JsonValue::Null);
    assert_eq!(
        build_json["interfaces"],
        serde_json::json!([
            {
                "manifest_path": package_manifest.display().to_string().replace('\\', "/"),
                "package_name": "demo-package",
                "selected": true,
                "status": "wrote",
                "path": package_interface_output.display().to_string().replace('\\', "/"),
            }
        ])
    );
    assert_stdlib_dependency_build_targets("initialized stdlib package build json", &build_json);
    assert_build_json_includes_target(
        "initialized stdlib package build json",
        &build_json,
        serde_json::json!({
            "manifest_path": package_manifest.display().to_string().replace('\\', "/"),
            "package_name": "demo-package",
            "selected": true,
            "dependency_only": false,
            "kind": "lib",
            "path": "src/lib.ql",
            "emit": "staticlib",
            "profile": "debug",
            "artifact_path": package_library_output.display().to_string().replace('\\', "/"),
            "c_header_path": JsonValue::Null,
        }),
    );
    assert_build_json_includes_target(
        "initialized stdlib package build json",
        &build_json,
        serde_json::json!({
            "manifest_path": package_manifest.display().to_string().replace('\\', "/"),
            "package_name": "demo-package",
            "selected": true,
            "dependency_only": false,
            "kind": "bin",
            "path": "src/main.ql",
            "emit": "llvm-ir",
            "profile": "debug",
            "artifact_path": package_build_output.display().to_string().replace('\\', "/"),
            "c_header_path": JsonValue::Null,
        }),
    );
    expect_file_exists(
        "project-init-stdlib-package-run",
        &package_library_output,
        "initialized stdlib package library artifact",
        "json build initialized stdlib package",
    )
    .unwrap();
    expect_file_exists(
        "project-init-stdlib-package-run",
        &package_build_output,
        "initialized stdlib package build artifact",
        "json build initialized stdlib package",
    )
    .unwrap();
    expect_file_exists(
        "project-init-stdlib-package-run",
        &package_interface_output,
        "initialized stdlib package interface artifact",
        "json build initialized stdlib package",
    )
    .unwrap();

    let mut run = ql_command(&workspace_root);
    run.current_dir(temp.path());
    run.args(["run"]).arg(&project_root);
    let output = run_command_capture(&mut run, "`ql run` initialized stdlib package");
    let (stdout, stderr) = expect_exit_code(
        "project-init-stdlib-package-run",
        "run initialized stdlib package",
        &output,
        0,
    )
    .unwrap();
    expect_silent_output(
        "project-init-stdlib-package-run",
        "run initialized stdlib package",
        &stdout,
        &stderr,
    )
    .unwrap();

    let mut run_json = ql_command(&workspace_root);
    run_json.current_dir(temp.path());
    run_json.args(["run"]).arg(&project_root).arg("--json");
    let output = run_command_capture(&mut run_json, "`ql run --json` initialized stdlib package");
    let (stdout, stderr) = expect_exit_code(
        "project-init-stdlib-package-run",
        "json run initialized stdlib package",
        &output,
        0,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-package-run",
        "json run initialized stdlib package",
        &stderr,
    )
    .unwrap();

    let run_json = parse_json_output("project-init-stdlib-package-run", &stdout);
    assert_eq!(run_json["schema"], "ql.run.v1");
    assert_eq!(
        run_json["path"],
        project_root.display().to_string().replace('\\', "/")
    );
    assert_eq!(run_json["scope"], "project");
    assert_eq!(
        run_json["project_manifest_path"],
        package_manifest.display().to_string().replace('\\', "/")
    );
    assert_eq!(run_json["requested_profile"], "debug");
    assert_eq!(run_json["profile_overridden"], false);
    assert_eq!(run_json["program_args"], serde_json::json!([]));
    assert_eq!(run_json["status"], "completed");
    assert_eq!(run_json["failure"], JsonValue::Null);
    assert_eq!(
        run_json["built_target"],
        serde_json::json!({
            "manifest_path": package_manifest.display().to_string().replace('\\', "/"),
            "package_name": "demo-package",
            "selected": true,
            "dependency_only": false,
            "kind": "bin",
            "path": "src/main.ql",
            "emit": "exe",
            "profile": "debug",
            "artifact_path": package_run_output.display().to_string().replace('\\', "/"),
            "c_header_path": JsonValue::Null,
        })
    );
    assert_eq!(
        run_json["execution"],
        serde_json::json!({
            "exit_code": 0,
            "stdout": "",
            "stderr": "",
        })
    );
    expect_file_exists(
        "project-init-stdlib-package-run",
        &package_run_output,
        "initialized stdlib package executable",
        "json run initialized stdlib package",
    )
    .unwrap();

    let mut test = ql_command(&workspace_root);
    test.current_dir(temp.path());
    test.args(["test"]).arg(&project_root);
    let output = run_command_capture(&mut test, "`ql test` initialized stdlib package");
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-package-run",
        "test initialized stdlib package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-package-run",
        "test initialized stdlib package",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "project-init-stdlib-package-run",
        &stdout.replace('\\', "/"),
        &[
            "test tests/smoke.ql ... ok",
            "test result: ok. 1 passed; 0 failed",
        ],
    )
    .unwrap();

    let mut test_json = ql_command(&workspace_root);
    test_json.current_dir(temp.path());
    test_json.args(["test", "--json"]).arg(&project_root);
    let output = run_command_capture(
        &mut test_json,
        "`ql test --json` initialized stdlib package",
    );
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-package-run",
        "json test initialized stdlib package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-package-run",
        "json test initialized stdlib package",
        &stderr,
    )
    .unwrap();

    let actual = parse_json_output("project-init-stdlib-package-run", &stdout);
    let expected = serde_json::json!({
        "schema": "ql.test.v1",
        "path": project_root.display().to_string().replace('\\', "/"),
        "requested_profile": "debug",
        "profile_overridden": false,
        "package_name": JsonValue::Null,
        "filter": JsonValue::Null,
        "list_only": false,
        "status": "ok",
        "discovered_total": 1,
        "selected_total": 1,
        "targets": [
            {
                "path": "tests/smoke.ql",
                "kind": "smoke",
                "profile": "debug",
            }
        ],
        "passed": 1,
        "failed": 0,
        "failures": [],
    });
    assert_eq!(
        actual, expected,
        "initialized stdlib package should keep a stable test json contract"
    );
}

#[test]
fn project_init_with_stdlib_creates_consuming_workspace_scaffold_and_check_succeeds() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-init-stdlib-workspace");
    let stdlib_root = write_repo_stdlib_fixture(&temp, &workspace_root);
    let project_root = temp.path().join("demo-workspace");
    let member_root = project_root.join("packages/app");

    let mut init = ql_command(&workspace_root);
    init.args([
        "project",
        "init",
        &project_root.to_string_lossy(),
        "--workspace",
        "--name",
        "app",
        "--stdlib",
        &stdlib_root.to_string_lossy(),
    ]);
    let output = run_command_capture(&mut init, "`ql project init --workspace --stdlib`");
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-workspace",
        "stdlib workspace init",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-workspace",
        "stdlib workspace init",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "project-init-stdlib-workspace",
        &stdout,
        &[
            &format!(
                "created: {}",
                project_root
                    .join("qlang.toml")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            &format!(
                "created: {}",
                member_root
                    .join("qlang.toml")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            &format!(
                "created: {}",
                member_root
                    .join("tests/smoke.ql")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
        ],
    )
    .unwrap();

    assert_eq!(
        read_normalized_file(
            &project_root.join("qlang.toml"),
            "stdlib workspace manifest"
        ),
        "[workspace]\nmembers = [\"packages/app\"]\n"
    );
    assert_eq!(
        read_normalized_file(
            &member_root.join("qlang.toml"),
            "stdlib workspace member manifest"
        ),
        "[package]\nname = \"app\"\n\n[dependencies]\n\"std.core\" = \"../../../stdlib/packages/core\"\n\"std.option\" = \"../../../stdlib/packages/option\"\n\"std.result\" = \"../../../stdlib/packages/result\"\n\"std.array\" = \"../../../stdlib/packages/array\"\n\"std.test\" = \"../../../stdlib/packages/test\"\n"
    );
    let lib_source = read_normalized_file(
        &member_root.join("src/lib.ql"),
        "stdlib workspace member source",
    );
    expect_stdlib_starter_source(&lib_source, "stdlib workspace member source");
    let main_source = read_normalized_file(
        &member_root.join("src/main.ql"),
        "stdlib workspace member main source",
    );
    expect_stdlib_starter_main_source(&main_source, "stdlib workspace member main source");
    let smoke_source = read_normalized_file(
        &member_root.join("tests/smoke.ql"),
        "stdlib workspace member smoke test",
    );
    expect_stdlib_starter_smoke_source(&smoke_source, "stdlib workspace member smoke test");

    let mut check = ql_command(&workspace_root);
    check.args(["check", &project_root.to_string_lossy()]);
    let output = run_command_capture(&mut check, "`ql check` initialized stdlib workspace");
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-workspace",
        "check initialized stdlib workspace",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-workspace",
        "check initialized stdlib workspace",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "project-init-stdlib-workspace",
        &stdout.replace('\\', "/"),
        &[
            &format!(
                "ok: {}",
                member_root
                    .join("src/lib.ql")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            "loaded interface:",
        ],
    )
    .unwrap();

    let mut check_json = ql_command(&workspace_root);
    check_json.args(["check", &project_root.to_string_lossy(), "--json"]);
    let output = run_command_capture(
        &mut check_json,
        "`ql check --json` initialized stdlib workspace",
    );
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-workspace",
        "json check initialized stdlib workspace",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-workspace",
        "json check initialized stdlib workspace",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("project-init-stdlib-workspace", &stdout);
    assert_stdlib_check_json(
        "initialized stdlib workspace check json",
        &actual,
        "workspace",
        &project_root.join("qlang.toml"),
        &[
            member_root.join("src/lib.ql"),
            member_root.join("src/main.ql"),
        ],
        &stdlib_root,
    );

    let member_interface = member_root.join("app.qi");
    let mut emit_interface = ql_command(&workspace_root);
    emit_interface.args([
        "project",
        "emit-interface",
        &project_root.to_string_lossy(),
        "--package",
        "app",
    ]);
    let output = run_command_capture(
        &mut emit_interface,
        "`ql project emit-interface --package app` initialized stdlib workspace",
    );
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-workspace",
        "emit interface initialized stdlib workspace package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-workspace",
        "emit interface initialized stdlib workspace package",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "project-init-stdlib-workspace",
        &stdout.replace('\\', "/"),
        &[&format!(
            "wrote interface: {}",
            member_interface.display().to_string().replace('\\', "/")
        )],
    )
    .unwrap();
    let interface_source = read_normalized_file(
        &member_interface,
        "initialized stdlib workspace member interface artifact",
    );
    expect_stdlib_starter_interface(
        &interface_source,
        "app",
        "initialized stdlib workspace member interface artifact",
    );
    expect_emit_interface_check_ok(
        "project-init-stdlib-workspace",
        &workspace_root,
        &project_root,
        Some("app"),
        &member_interface,
        "`ql project emit-interface --check --package app` initialized stdlib workspace",
    );

    let mut graph_json = ql_command(&workspace_root);
    graph_json.args([
        "project",
        "graph",
        &project_root.to_string_lossy(),
        "--package",
        "app",
        "--json",
    ]);
    let output = run_command_capture(
        &mut graph_json,
        "`ql project graph --json --package app` initialized stdlib workspace",
    );
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-workspace",
        "graph json initialized stdlib workspace package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-workspace",
        "graph json initialized stdlib workspace package",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("project-init-stdlib-workspace", &stdout);
    assert_stdlib_workspace_graph_json(
        "initialized stdlib workspace graph json",
        &actual,
        &member_root.join("qlang.toml"),
    );

    let mut status_json = ql_command(&workspace_root);
    status_json.args([
        "project",
        "status",
        &project_root.to_string_lossy(),
        "--package",
        "app",
        "--json",
    ]);
    let output = run_command_capture(
        &mut status_json,
        "`ql project status --json --package app` initialized stdlib workspace",
    );
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-workspace",
        "status json initialized stdlib workspace package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-workspace",
        "status json initialized stdlib workspace package",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("project-init-stdlib-workspace", &stdout);
    assert_stdlib_workspace_status_json(
        "initialized stdlib workspace status json",
        &actual,
        &project_root,
        &member_root,
    );

    let mut dependencies = ql_command(&workspace_root);
    dependencies.args([
        "project",
        "dependencies",
        &project_root.to_string_lossy(),
        "--name",
        "app",
    ]);
    let output = run_command_capture(
        &mut dependencies,
        "`ql project dependencies --name app` initialized stdlib workspace",
    );
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-workspace",
        "dependencies initialized stdlib workspace package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-workspace",
        "dependencies initialized stdlib workspace package",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "project-init-stdlib-workspace",
        &stdout.replace('\\', "/"),
        &[
            &format!(
                "workspace_manifest: {}",
                project_root
                    .join("qlang.toml")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
            "package: app",
            "dependencies:",
            "  - ../../../stdlib/packages/core (std.core, local)",
            "  - ../../../stdlib/packages/option (std.option, local)",
            "  - ../../../stdlib/packages/result (std.result, local)",
            "  - ../../../stdlib/packages/array (std.array, local)",
            "  - ../../../stdlib/packages/test (std.test, local)",
        ],
    )
    .unwrap();

    let mut dependencies_json = ql_command(&workspace_root);
    dependencies_json.args([
        "project",
        "dependencies",
        &project_root.to_string_lossy(),
        "--name",
        "app",
        "--json",
    ]);
    let output = run_command_capture(
        &mut dependencies_json,
        "`ql project dependencies --json --name app` initialized stdlib workspace",
    );
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-workspace",
        "dependencies json initialized stdlib workspace package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-workspace",
        "dependencies json initialized stdlib workspace package",
        &stderr,
    )
    .unwrap();
    let actual = parse_json_output("project-init-stdlib-workspace", &stdout);
    assert_stdlib_dependencies_json(
        "initialized stdlib workspace dependencies json",
        &actual,
        &project_root,
        &project_root.join("qlang.toml"),
        "app",
        "../../../stdlib/packages",
        &stdlib_root,
    );
}

#[test]
fn project_init_with_stdlib_creates_runnable_and_testable_workspace_scaffold() {
    if !toolchain_available("`ql project init --workspace --stdlib` runnable workspace test") {
        return;
    }

    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-cli-project-init-stdlib-workspace-run");
    let stdlib_root = write_repo_stdlib_fixture(&temp, &workspace_root);
    let project_root = temp.path().join("demo-workspace");
    let member_root = project_root.join("packages/app");
    let app_manifest = member_root.join("qlang.toml");
    let app_library_output =
        static_library_output_path(&member_root.join("target/ql/debug"), "lib");
    let app_build_output = member_root.join("target/ql/debug/main.ll");
    let app_output = executable_output_path(&member_root.join("target/ql/debug"), "main");
    let app_interface_output = member_root.join("app.qi");

    let mut init = ql_command(&workspace_root);
    init.args([
        "project",
        "init",
        &project_root.to_string_lossy(),
        "--workspace",
        "--name",
        "app",
        "--stdlib",
        &stdlib_root.to_string_lossy(),
    ]);
    let output = run_command_capture(
        &mut init,
        "`ql project init --workspace --stdlib` runnable workspace",
    );
    let (_stdout, stderr) = expect_success(
        "project-init-stdlib-workspace-run",
        "stdlib workspace init for runnable scaffold",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-workspace-run",
        "stdlib workspace init for runnable scaffold",
        &stderr,
    )
    .unwrap();

    let mut build_json = ql_command(&workspace_root);
    build_json.current_dir(temp.path());
    build_json
        .args(["build"])
        .arg(&project_root)
        .args(["--package", "app", "--json"]);
    let output = run_command_capture(
        &mut build_json,
        "`ql build --json --package app` initialized stdlib workspace",
    );
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-workspace-run",
        "json build initialized stdlib workspace package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-workspace-run",
        "json build initialized stdlib workspace package",
        &stderr,
    )
    .unwrap();

    let build_json = parse_json_output("project-init-stdlib-workspace-run", &stdout);
    assert_eq!(build_json["schema"], "ql.build.v1");
    assert_eq!(
        build_json["path"],
        project_root.display().to_string().replace('\\', "/")
    );
    assert_eq!(build_json["scope"], "project");
    assert_eq!(
        build_json["project_manifest_path"],
        project_root
            .join("qlang.toml")
            .display()
            .to_string()
            .replace('\\', "/")
    );
    assert_eq!(build_json["requested_emit"], "llvm-ir");
    assert_eq!(build_json["requested_profile"], "debug");
    assert_eq!(build_json["profile_overridden"], false);
    assert_eq!(build_json["emit_interface"], false);
    assert_eq!(build_json["status"], "ok");
    assert_eq!(build_json["failure"], JsonValue::Null);
    assert_eq!(
        build_json["interfaces"],
        serde_json::json!([
            {
                "manifest_path": app_manifest.display().to_string().replace('\\', "/"),
                "package_name": "app",
                "selected": true,
                "status": "wrote",
                "path": app_interface_output.display().to_string().replace('\\', "/"),
            }
        ])
    );
    assert_stdlib_dependency_build_targets("initialized stdlib workspace build json", &build_json);
    assert_build_json_includes_target(
        "initialized stdlib workspace build json",
        &build_json,
        serde_json::json!({
            "manifest_path": app_manifest.display().to_string().replace('\\', "/"),
            "package_name": "app",
            "selected": true,
            "dependency_only": false,
            "kind": "lib",
            "path": "src/lib.ql",
            "emit": "staticlib",
            "profile": "debug",
            "artifact_path": app_library_output.display().to_string().replace('\\', "/"),
            "c_header_path": JsonValue::Null,
        }),
    );
    assert_build_json_includes_target(
        "initialized stdlib workspace build json",
        &build_json,
        serde_json::json!({
            "manifest_path": app_manifest.display().to_string().replace('\\', "/"),
            "package_name": "app",
            "selected": true,
            "dependency_only": false,
            "kind": "bin",
            "path": "src/main.ql",
            "emit": "llvm-ir",
            "profile": "debug",
            "artifact_path": app_build_output.display().to_string().replace('\\', "/"),
            "c_header_path": JsonValue::Null,
        }),
    );
    expect_file_exists(
        "project-init-stdlib-workspace-run",
        &app_library_output,
        "initialized stdlib workspace library artifact",
        "json build initialized stdlib workspace package",
    )
    .unwrap();
    expect_file_exists(
        "project-init-stdlib-workspace-run",
        &app_build_output,
        "initialized stdlib workspace build artifact",
        "json build initialized stdlib workspace package",
    )
    .unwrap();
    expect_file_exists(
        "project-init-stdlib-workspace-run",
        &app_interface_output,
        "initialized stdlib workspace interface artifact",
        "json build initialized stdlib workspace package",
    )
    .unwrap();

    let mut run = ql_command(&workspace_root);
    run.current_dir(temp.path());
    run.args(["run"]).arg(&project_root);
    let output = run_command_capture(&mut run, "`ql run` initialized stdlib workspace");
    let (stdout, stderr) = expect_exit_code(
        "project-init-stdlib-workspace-run",
        "run initialized stdlib workspace",
        &output,
        0,
    )
    .unwrap();
    expect_silent_output(
        "project-init-stdlib-workspace-run",
        "run initialized stdlib workspace",
        &stdout,
        &stderr,
    )
    .unwrap();

    let mut run_json = ql_command(&workspace_root);
    run_json.current_dir(temp.path());
    run_json
        .args(["run"])
        .arg(&project_root)
        .args(["--package", "app", "--json"]);
    let output = run_command_capture(
        &mut run_json,
        "`ql run --json --package app` initialized stdlib workspace",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-init-stdlib-workspace-run",
        "json run initialized stdlib workspace package",
        &output,
        0,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-workspace-run",
        "json run initialized stdlib workspace package",
        &stderr,
    )
    .unwrap();
    let run_json = parse_json_output("project-init-stdlib-workspace-run", &stdout);
    assert_eq!(run_json["schema"], "ql.run.v1");
    assert_eq!(
        run_json["path"],
        project_root.display().to_string().replace('\\', "/")
    );
    assert_eq!(run_json["scope"], "project");
    assert_eq!(
        run_json["project_manifest_path"],
        project_root
            .join("qlang.toml")
            .display()
            .to_string()
            .replace('\\', "/")
    );
    assert_eq!(run_json["requested_profile"], "debug");
    assert_eq!(run_json["profile_overridden"], false);
    assert_eq!(run_json["program_args"], serde_json::json!([]));
    assert_eq!(run_json["status"], "completed");
    assert_eq!(run_json["failure"], JsonValue::Null);
    assert_eq!(
        run_json["built_target"],
        serde_json::json!({
            "manifest_path": member_root.join("qlang.toml").display().to_string().replace('\\', "/"),
            "package_name": "app",
            "selected": true,
            "dependency_only": false,
            "kind": "bin",
            "path": "src/main.ql",
            "emit": "exe",
            "profile": "debug",
            "artifact_path": app_output.display().to_string().replace('\\', "/"),
            "c_header_path": JsonValue::Null,
        })
    );
    assert_eq!(
        run_json["execution"],
        serde_json::json!({
            "exit_code": 0,
            "stdout": "",
            "stderr": "",
        })
    );
    expect_file_exists(
        "project-init-stdlib-workspace-run",
        &app_output,
        "initialized stdlib workspace executable",
        "json run initialized stdlib workspace package",
    )
    .unwrap();

    let mut test = ql_command(&workspace_root);
    test.current_dir(temp.path());
    test.args(["test"]).arg(&project_root);
    let output = run_command_capture(&mut test, "`ql test` initialized stdlib workspace");
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-workspace-run",
        "test initialized stdlib workspace",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-workspace-run",
        "test initialized stdlib workspace",
        &stderr,
    )
    .unwrap();
    expect_stdout_contains_all(
        "project-init-stdlib-workspace-run",
        &stdout.replace('\\', "/"),
        &[
            "test packages/app/tests/smoke.ql ... ok",
            "test result: ok. 1 passed; 0 failed",
        ],
    )
    .unwrap();

    let mut test_json = ql_command(&workspace_root);
    test_json.current_dir(temp.path());
    test_json
        .args(["test", "--json"])
        .arg(&project_root)
        .args(["--package", "app"]);
    let output = run_command_capture(
        &mut test_json,
        "`ql test --json --package app` initialized stdlib workspace",
    );
    let (stdout, stderr) = expect_success(
        "project-init-stdlib-workspace-run",
        "json test initialized stdlib workspace package",
        &output,
    )
    .unwrap();
    expect_empty_stderr(
        "project-init-stdlib-workspace-run",
        "json test initialized stdlib workspace package",
        &stderr,
    )
    .unwrap();

    let actual = parse_json_output("project-init-stdlib-workspace-run", &stdout);
    let expected = serde_json::json!({
        "schema": "ql.test.v1",
        "path": project_root.display().to_string().replace('\\', "/"),
        "requested_profile": "debug",
        "profile_overridden": false,
        "package_name": "app",
        "filter": JsonValue::Null,
        "list_only": false,
        "status": "ok",
        "discovered_total": 1,
        "selected_total": 1,
        "targets": [
            {
                "path": "packages/app/tests/smoke.ql",
                "kind": "smoke",
                "profile": "debug",
            }
        ],
        "passed": 1,
        "failed": 0,
        "failures": [],
    });
    assert_eq!(
        actual, expected,
        "initialized stdlib workspace should keep a stable package-selected test json contract"
    );
}
