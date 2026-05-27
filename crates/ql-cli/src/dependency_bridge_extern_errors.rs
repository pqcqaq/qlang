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
#[path = "dependency_bridge_extern_errors_tests.rs"]
mod tests;
