use std::path::Path;

use crate::build_plan::{PrepareProjectTargetBuildError, PrepareProjectTargetBuildFailureKind};
use crate::dependency_bridge_names::DependencyExternOwner;
use crate::dependency_bridge_reporting::{
    report_direct_dependency_local_conflict, report_direct_dependency_symbol_conflict,
};

pub(crate) enum DependencyPublicValueBridgeError {
    DependencyConflict {
        symbol: String,
        owner: DependencyExternOwner,
    },
    LocalConflict {
        symbol: String,
    },
}

pub(crate) fn report_direct_dependency_value_bridge_error(
    command_label: &str,
    dependency_package: &str,
    error: DependencyPublicValueBridgeError,
) {
    match error {
        DependencyPublicValueBridgeError::DependencyConflict { symbol, owner } => {
            report_direct_dependency_symbol_conflict(
                command_label,
                "public value",
                &symbol,
                &owner.package_name,
                dependency_package,
                "keep direct dependency public value names unique until package-qualified dependency value lowering lands",
            );
        }
        DependencyPublicValueBridgeError::LocalConflict { symbol } => {
            report_direct_dependency_local_conflict(
                command_label,
                "public value",
                &symbol,
                dependency_package,
                "rename the local top-level item or avoid importing a direct dependency public value with the same original symbol name",
            );
        }
    }
}

pub(crate) fn dependency_value_bridge_target_prep_error(
    error: DependencyPublicValueBridgeError,
    dependency_package: &str,
    dependency_manifest_path: &Path,
) -> PrepareProjectTargetBuildError {
    let failure_kind = match error {
        DependencyPublicValueBridgeError::DependencyConflict { symbol, owner } => {
            PrepareProjectTargetBuildFailureKind::DependencyValueConflict {
                symbol,
                first_package: owner.package_name,
                first_manifest_path: owner.manifest_path,
                conflicting_package: dependency_package.to_owned(),
                conflicting_manifest_path: dependency_manifest_path.to_path_buf(),
            }
        }
        DependencyPublicValueBridgeError::LocalConflict { symbol } => {
            PrepareProjectTargetBuildFailureKind::DependencyValueLocalConflict {
                symbol,
                dependency_package: dependency_package.to_owned(),
                dependency_manifest_path: dependency_manifest_path.to_path_buf(),
            }
        }
    };
    PrepareProjectTargetBuildError { failure_kind }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

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
}
