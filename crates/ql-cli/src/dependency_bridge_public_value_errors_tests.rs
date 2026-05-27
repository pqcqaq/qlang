use std::path::{Path, PathBuf};

use super::*;

#[test]
fn value_bridge_target_prep_error_preserves_local_conflict_context() {
    let error = dependency_value_bridge_target_prep_error(
        DependencyPublicValueBridgeError::LocalConflict {
            symbol: "VALUE".to_owned(),
        },
        "dep",
        Path::new("workspace/dep/qlang.toml"),
    );

    match error.failure_kind {
        PrepareProjectTargetBuildFailureKind::DependencyValueLocalConflict {
            symbol,
            dependency_package,
            dependency_manifest_path,
        } => {
            assert_eq!(symbol, "VALUE");
            assert_eq!(dependency_package, "dep");
            assert_eq!(
                dependency_manifest_path,
                PathBuf::from("workspace/dep/qlang.toml")
            );
        }
        _ => panic!("expected dependency value local conflict"),
    }
}

#[test]
fn value_bridge_target_prep_error_preserves_dependency_conflict_context() {
    let error = dependency_value_bridge_target_prep_error(
        DependencyPublicValueBridgeError::DependencyConflict {
            symbol: "VALUE".to_owned(),
            owner: DependencyExternOwner {
                package_name: "first".to_owned(),
                manifest_path: PathBuf::from("workspace/first/qlang.toml"),
            },
        },
        "second",
        Path::new("workspace/second/qlang.toml"),
    );

    match error.failure_kind {
        PrepareProjectTargetBuildFailureKind::DependencyValueConflict {
            symbol,
            first_package,
            first_manifest_path,
            conflicting_package,
            conflicting_manifest_path,
        } => {
            assert_eq!(symbol, "VALUE");
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
        _ => panic!("expected dependency value conflict"),
    }
}
