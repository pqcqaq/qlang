use std::path::PathBuf;

use ql_driver::{BuildError, ToolchainError};
use ql_project::{BuildTargetKind, ManifestBuildProfile};

use crate::project_interfaces::ReferenceInterfacePrepFailure;

use super::*;

#[test]
fn build_json_failure_preserves_toolchain_target_context_and_artifacts() {
    let failure = build_json_failure(
        Some(Path::new("workspace/app/qlang.toml")),
        Some("app"),
        "bin",
        "src/main.ql".to_owned(),
        false,
        &BuildError::Toolchain {
            error: ToolchainError::InvocationFailed {
                program: "clang".to_owned(),
                status: Some(7),
                stderr: "mock link failure".to_owned(),
            },
            preserved_artifacts: vec![
                PathBuf::from("target/ql/debug/main.123.codegen.ll"),
                PathBuf::from(if cfg!(windows) {
                    "target/ql/debug/main.123.codegen.obj"
                } else {
                    "target/ql/debug/main.123.codegen.o"
                }),
            ],
        },
    );

    assert_eq!(failure["manifest_path"], "workspace/app/qlang.toml");
    assert_eq!(failure["package_name"], "app");
    assert_eq!(failure["selected"], false);
    assert_eq!(failure["dependency_only"], true);
    assert_eq!(failure["kind"], "bin");
    assert_eq!(failure["path"], "src/main.ql");
    assert_eq!(failure["error_kind"], "toolchain");
    assert_eq!(
        failure["message"],
        "toolchain `clang` failed with exit code 7: mock link failure"
    );
    assert_eq!(
        failure["intermediate_ir"],
        "target/ql/debug/main.123.codegen.ll"
    );
    assert_eq!(
        failure["preserved_artifacts"]
            .as_array()
            .expect("preserved artifacts should be an array")
            .len(),
        2
    );
}

#[test]
fn build_json_preflight_failure_preserves_nullable_target_context() {
    let failure = build_json_preflight_failure(
        Path::new("workspace"),
        Some(Path::new("workspace/qlang.toml")),
        Some("app"),
        Some(true),
        "selector",
        "target-selection",
        "target selector matched no build targets".to_owned(),
        Some("target `missing.ql`".to_owned()),
        Some("workspace/src/missing.ql".to_owned()),
        Some(0),
    );

    assert_eq!(failure["manifest_path"], "workspace/qlang.toml");
    assert_eq!(failure["package_name"], "app");
    assert_eq!(failure["selected"], true);
    assert_eq!(failure["dependency_only"], false);
    assert!(failure["kind"].is_null());
    assert_eq!(failure["path"], "workspace");
    assert_eq!(failure["error_kind"], "selector");
    assert_eq!(failure["stage"], "target-selection");
    assert_eq!(failure["selector"], "target `missing.ql`");
    assert_eq!(failure["conflict_path"], "workspace/src/missing.ql");
    assert_eq!(failure["target_count"], 0);
}

#[test]
fn build_json_emit_interface_failure_preserves_interface_target_context() {
    let failure = build_json_emit_interface_failure(
        Path::new("workspace/app"),
        Some(Path::new("workspace/app/qlang.toml")),
        Some("app"),
        &EmitPackageInterfaceError::OutputPathFailure {
            manifest_path: Some(PathBuf::from("workspace/app/qlang.toml")),
            output_path: PathBuf::from("workspace/app/app.qi"),
            message: "failed to write interface".to_owned(),
        },
    );

    assert_eq!(failure["manifest_path"], "workspace/app/qlang.toml");
    assert_eq!(failure["package_name"], "app");
    assert_eq!(failure["selected"], true);
    assert_eq!(failure["dependency_only"], false);
    assert_eq!(failure["kind"], "interface");
    assert_eq!(failure["path"], "workspace/app/app.qi");
    assert_eq!(failure["error_kind"], "interface-output");
    assert_eq!(failure["stage"], "emit-interface");
    assert_eq!(failure["message"], "failed to write interface");
    assert_eq!(failure["output_path"], "workspace/app/app.qi");
    assert!(failure["source_root"].is_null());
    assert!(failure["failing_source_count"].is_null());
    assert!(failure["first_failing_source"].is_null());
}

#[test]
fn build_json_dependency_interface_prep_failure_preserves_dependency_context() {
    let failure = build_json_dependency_interface_prep_failure(
        Path::new("workspace/app"),
        &ReferenceInterfacePrepError {
            failure_count: 2,
            first_failure_manifest: Some(PathBuf::from("workspace/lib/qlang.toml")),
            first_failure: ReferenceInterfacePrepFailure {
                owner_manifest_path: Some(PathBuf::from("workspace/app/qlang.toml")),
                reference: Some("lib".to_owned()),
                reference_manifest_path: PathBuf::from("workspace/lib/qlang.toml"),
                manifest_path: Some(PathBuf::from("workspace/lib/qlang.toml")),
                failure_kind: ReferenceInterfacePrepFailureKind::InterfaceEmit(
                    EmitPackageInterfaceError::NoSourceFilesFailure {
                        manifest_path: PathBuf::from("workspace/lib/qlang.toml"),
                        source_root: PathBuf::from("workspace/lib/src"),
                    },
                ),
            },
        },
    );

    assert_eq!(failure["manifest_path"], "workspace/lib/qlang.toml");
    assert!(failure["package_name"].is_null());
    assert_eq!(failure["selected"], false);
    assert_eq!(failure["dependency_only"], true);
    assert_eq!(failure["kind"], "interface");
    assert_eq!(failure["path"], "workspace/lib/src");
    assert_eq!(failure["error_kind"], "package-sources");
    assert_eq!(failure["stage"], "dependency-interface-prep");
    assert_eq!(failure["source_root"], "workspace/lib/src");
    assert!(failure["output_path"].is_null());
    assert!(failure["failing_source_count"].is_null());
    assert_eq!(failure["owner_manifest_path"], "workspace/app/qlang.toml");
    assert_eq!(
        failure["reference_manifest_path"],
        "workspace/lib/qlang.toml"
    );
    assert_eq!(failure["reference"], "lib");
    assert_eq!(failure["failing_dependency_count"], 2);
    assert_eq!(
        failure["first_failing_dependency_manifest"],
        "workspace/lib/qlang.toml"
    );
}

#[test]
fn build_json_build_plan_failure_preserves_cycle_context() {
    let failure = build_json_build_plan_failure(
        Path::new("workspace/app"),
        &BuildPlanResolveError {
            manifest_path: Some(PathBuf::from("workspace/app/qlang.toml")),
            owner_manifest_path: None,
            dependency_manifest_path: Some(PathBuf::from("workspace/app/qlang.toml")),
            failure_kind: BuildPlanResolveFailureKind::Cycle {
                cycle_manifests: vec![
                    "workspace/app/qlang.toml".to_owned(),
                    "workspace/lib/qlang.toml".to_owned(),
                    "workspace/app/qlang.toml".to_owned(),
                ],
            },
        },
    );

    assert_eq!(failure["manifest_path"], "workspace/app/qlang.toml");
    assert!(failure["package_name"].is_null());
    assert!(failure["selected"].is_null());
    assert!(failure["dependency_only"].is_null());
    assert!(failure["kind"].is_null());
    assert_eq!(failure["path"], "workspace/app");
    assert_eq!(failure["error_kind"], "cycle");
    assert_eq!(failure["stage"], "build-plan");
    assert!(failure["owner_manifest_path"].is_null());
    assert_eq!(
        failure["dependency_manifest_path"],
        "workspace/app/qlang.toml"
    );
    assert_eq!(
        failure["cycle_manifests"],
        json!([
            "workspace/app/qlang.toml",
            "workspace/lib/qlang.toml",
            "workspace/app/qlang.toml"
        ])
    );
}

#[test]
fn build_json_target_prep_failure_preserves_target_context_and_detail_fields() {
    let member = WorkspaceBuildTargets {
        member_manifest_path: PathBuf::from("workspace/app/qlang.toml"),
        package_name: "app".to_owned(),
        default_profile: Some(ManifestBuildProfile::Debug),
        targets: Vec::new(),
    };
    let target = BuildTarget {
        kind: BuildTargetKind::Binary,
        path: PathBuf::from("workspace/app/src/main.ql"),
    };
    let failure = build_json_target_prep_failure(
        &member,
        &target,
        false,
        &PrepareProjectTargetBuildError {
            failure_kind: PrepareProjectTargetBuildFailureKind::DependencyInterface {
                dependency_manifest_path: PathBuf::from("workspace/lib/qlang.toml"),
                dependency_package: "lib".to_owned(),
                interface_path: PathBuf::from("workspace/lib/lib.qi"),
                message: "missing dependency interface".to_owned(),
            },
        },
    );

    assert_eq!(failure["manifest_path"], "workspace/app/qlang.toml");
    assert_eq!(failure["package_name"], "app");
    assert_eq!(failure["selected"], false);
    assert_eq!(failure["dependency_only"], true);
    assert_eq!(failure["kind"], "bin");
    assert_eq!(failure["path"], "src/main.ql");
    assert_eq!(failure["error_kind"], "dependency-interface");
    assert_eq!(failure["stage"], "target-prep");
    assert_eq!(failure["message"], "missing dependency interface");
    assert_eq!(
        failure["dependency_manifest_path"],
        "workspace/lib/qlang.toml"
    );
    assert_eq!(failure["dependency_package"], "lib");
    assert_eq!(failure["interface_path"], "workspace/lib/lib.qi");
    assert!(failure["symbol"].is_null());
    assert!(failure["io_path"].is_null());
}

#[test]
fn build_json_target_prep_failure_preserves_conflict_detail_fields() {
    let member = WorkspaceBuildTargets {
        member_manifest_path: PathBuf::from("workspace/app/qlang.toml"),
        package_name: "app".to_owned(),
        default_profile: Some(ManifestBuildProfile::Debug),
        targets: Vec::new(),
    };
    let target = BuildTarget {
        kind: BuildTargetKind::Library,
        path: PathBuf::from("workspace/app/src/lib.ql"),
    };
    let failure = build_json_target_prep_failure(
        &member,
        &target,
        true,
        &PrepareProjectTargetBuildError {
            failure_kind: PrepareProjectTargetBuildFailureKind::DependencyTypeConflict {
                symbol: "SharedType".to_owned(),
                first_package: "dep_a".to_owned(),
                first_manifest_path: PathBuf::from("workspace/dep_a/qlang.toml"),
                conflicting_package: "dep_b".to_owned(),
                conflicting_manifest_path: PathBuf::from("workspace/dep_b/qlang.toml"),
            },
        },
    );

    assert_eq!(failure["selected"], true);
    assert_eq!(failure["dependency_only"], false);
    assert_eq!(failure["kind"], "lib");
    assert_eq!(failure["path"], "src/lib.ql");
    assert_eq!(failure["error_kind"], "dependency-type-conflict");
    assert_eq!(failure["symbol"], "SharedType");
    assert_eq!(failure["first_dependency_package"], "dep_a");
    assert_eq!(
        failure["first_dependency_manifest_path"],
        "workspace/dep_a/qlang.toml"
    );
    assert_eq!(failure["conflicting_dependency_package"], "dep_b");
    assert_eq!(
        failure["conflicting_dependency_manifest_path"],
        "workspace/dep_b/qlang.toml"
    );
    assert!(failure["dependency_manifest_path"].is_null());
    assert!(failure["io_path"].is_null());
}

#[test]
fn build_json_target_prep_failure_preserves_source_read_io_path() {
    let member = WorkspaceBuildTargets {
        member_manifest_path: PathBuf::from("workspace/app/qlang.toml"),
        package_name: "app".to_owned(),
        default_profile: Some(ManifestBuildProfile::Debug),
        targets: Vec::new(),
    };
    let target = BuildTarget {
        kind: BuildTargetKind::Source,
        path: PathBuf::from("workspace/app/src/main.ql"),
    };
    let failure = build_json_target_prep_failure(
        &member,
        &target,
        true,
        &PrepareProjectTargetBuildError {
            failure_kind: PrepareProjectTargetBuildFailureKind::SourceRead {
                path: PathBuf::from("workspace/app/src/main.ql"),
                message: "failed to read source".to_owned(),
            },
        },
    );

    assert_eq!(failure["kind"], "source");
    assert_eq!(failure["error_kind"], "io");
    assert_eq!(failure["message"], "failed to read source");
    assert_eq!(failure["io_path"], "workspace/app/src/main.ql");
    assert!(failure["dependency_manifest_path"].is_null());
    assert!(failure["dependency_package"].is_null());
    assert!(failure["symbol"].is_null());
}
