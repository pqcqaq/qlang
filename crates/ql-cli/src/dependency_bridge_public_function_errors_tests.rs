use std::path::{Path, PathBuf};

use super::*;

#[test]
fn function_forwarder_target_prep_error_preserves_local_conflict_context() {
    let error = dependency_function_forwarder_target_prep_error(
        DependencyPublicFunctionForwarderError::LocalConflict {
            symbol: "add".to_owned(),
        },
        "dep",
        Path::new("workspace/dep/qlang.toml"),
    );

    match error.failure_kind {
        PrepareProjectTargetBuildFailureKind::DependencyFunctionLocalConflict {
            symbol,
            dependency_package,
            dependency_manifest_path,
        } => {
            assert_eq!(symbol, "add");
            assert_eq!(dependency_package, "dep");
            assert_eq!(
                dependency_manifest_path,
                PathBuf::from("workspace/dep/qlang.toml")
            );
        }
        _ => panic!("expected dependency function local conflict"),
    }
}

#[test]
fn function_forwarder_target_prep_error_preserves_dependency_conflict_context() {
    let error = dependency_function_forwarder_target_prep_error(
        DependencyPublicFunctionForwarderError::DependencyConflict {
            symbol: "add".to_owned(),
            owner: DependencyExternOwner {
                package_name: "first".to_owned(),
                manifest_path: PathBuf::from("workspace/first/qlang.toml"),
            },
        },
        "second",
        Path::new("workspace/second/qlang.toml"),
    );

    match error.failure_kind {
        PrepareProjectTargetBuildFailureKind::DependencyFunctionConflict {
            symbol,
            first_package,
            first_manifest_path,
            conflicting_package,
            conflicting_manifest_path,
        } => {
            assert_eq!(symbol, "add");
            assert_eq!(first_package, "first");
            assert_eq!(
                first_manifest_path,
                PathBuf::from("workspace/first/qlang.toml")
            );
            assert_eq!(conflicting_package, "second");
            assert_eq!(
                conflicting_manifest_path,
                PathBuf::from("workspace/second/qlang.toml")
            );
        }
        _ => panic!("expected dependency function conflict"),
    }
}
