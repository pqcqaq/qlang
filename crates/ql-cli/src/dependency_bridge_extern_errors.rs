use std::path::Path;

use crate::build_plan::{PrepareProjectTargetBuildError, PrepareProjectTargetBuildFailureKind};
use crate::dependency_bridge_names::DependencyExternOwner;
use crate::dependency_bridge_reporting::report_direct_dependency_symbol_conflict;

#[derive(Debug)]
pub(crate) enum DependencyExternBridgeError {
    DependencyConflict {
        symbol: String,
        owner: DependencyExternOwner,
    },
}

pub(crate) fn report_direct_dependency_extern_bridge_error(
    command_label: &str,
    dependency_package: &str,
    error: DependencyExternBridgeError,
) {
    match error {
        DependencyExternBridgeError::DependencyConflict { symbol, owner } => {
            report_direct_dependency_symbol_conflict(
                command_label,
                "extern",
                &symbol,
                &owner.package_name,
                dependency_package,
                "keep direct dependency `extern \"c\"` names unique until package-qualified extern resolution lands",
            );
        }
    }
}

pub(crate) fn dependency_extern_bridge_target_prep_error(
    error: DependencyExternBridgeError,
    dependency_package: &str,
    dependency_manifest_path: &Path,
) -> PrepareProjectTargetBuildError {
    let failure_kind = match error {
        DependencyExternBridgeError::DependencyConflict { symbol, owner } => {
            PrepareProjectTargetBuildFailureKind::DependencyExternConflict {
                symbol,
                first_package: owner.package_name,
                first_manifest_path: owner.manifest_path,
                conflicting_package: dependency_package.to_owned(),
                conflicting_manifest_path: dependency_manifest_path.to_path_buf(),
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
    fn extern_bridge_target_prep_error_preserves_dependency_conflict_context() {
        let error = dependency_extern_bridge_target_prep_error(
            DependencyExternBridgeError::DependencyConflict {
                symbol: "q_add".to_owned(),
                owner: DependencyExternOwner {
                    package_name: "first".to_owned(),
                    manifest_path: PathBuf::from("workspace/first/qlang.toml"),
                },
            },
            "second",
            Path::new("workspace/second/qlang.toml"),
        );

        match error.failure_kind {
            PrepareProjectTargetBuildFailureKind::DependencyExternConflict {
                symbol,
                first_package,
                first_manifest_path,
                conflicting_package,
                conflicting_manifest_path,
            } => {
                assert_eq!(symbol, "q_add");
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
            _ => panic!("expected dependency extern conflict"),
        }
    }
}
