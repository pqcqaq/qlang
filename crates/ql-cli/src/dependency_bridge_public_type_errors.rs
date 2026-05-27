use std::path::Path;

use crate::build_plan::{PrepareProjectTargetBuildError, PrepareProjectTargetBuildFailureKind};
use crate::dependency_bridge_names::DependencyExternOwner;
use crate::dependency_bridge_reporting::{
    report_direct_dependency_local_conflict, report_direct_dependency_symbol_conflict,
    report_package_under_test_local_conflict, report_package_under_test_symbol_conflict,
};

pub(crate) enum DependencyPublicTypeBridgeError {
    DependencyConflict {
        symbol: String,
        owner: DependencyExternOwner,
    },
    LocalConflict {
        symbol: String,
    },
}

pub(crate) fn report_direct_dependency_type_bridge_error(
    command_label: &str,
    dependency_package: &str,
    error: DependencyPublicTypeBridgeError,
) {
    match error {
        DependencyPublicTypeBridgeError::DependencyConflict { symbol, owner } => {
            report_direct_dependency_symbol_conflict(
                command_label,
                "public type",
                &symbol,
                &owner.package_name,
                dependency_package,
                "keep direct dependency public type names unique until package-qualified dependency type lowering lands",
            );
        }
        DependencyPublicTypeBridgeError::LocalConflict { symbol } => {
            report_direct_dependency_local_conflict(
                command_label,
                "public type",
                &symbol,
                dependency_package,
                "rename the local top-level item or avoid importing a direct dependency public type with the same original symbol name",
            );
        }
    }
}

pub(crate) fn report_package_under_test_type_bridge_error(
    command_label: &str,
    package_name: &str,
    error: DependencyPublicTypeBridgeError,
) {
    match error {
        DependencyPublicTypeBridgeError::DependencyConflict { symbol, owner } => {
            report_package_under_test_symbol_conflict(
                command_label,
                "public type",
                &symbol,
                &owner.package_name,
                package_name,
            );
        }
        DependencyPublicTypeBridgeError::LocalConflict { symbol } => {
            report_package_under_test_local_conflict(
                command_label,
                "public type",
                &symbol,
                "rename the local top-level item or avoid importing a package-under-test public type with the same original symbol name",
            );
        }
    }
}

pub(crate) fn dependency_type_bridge_target_prep_error(
    error: DependencyPublicTypeBridgeError,
    dependency_package: &str,
    dependency_manifest_path: &Path,
) -> PrepareProjectTargetBuildError {
    let failure_kind = match error {
        DependencyPublicTypeBridgeError::DependencyConflict { symbol, owner } => {
            PrepareProjectTargetBuildFailureKind::DependencyTypeConflict {
                symbol,
                first_package: owner.package_name,
                first_manifest_path: owner.manifest_path,
                conflicting_package: dependency_package.to_owned(),
                conflicting_manifest_path: dependency_manifest_path.to_path_buf(),
            }
        }
        DependencyPublicTypeBridgeError::LocalConflict { symbol } => {
            PrepareProjectTargetBuildFailureKind::DependencyTypeLocalConflict {
                symbol,
                dependency_package: dependency_package.to_owned(),
                dependency_manifest_path: dependency_manifest_path.to_path_buf(),
            }
        }
    };
    PrepareProjectTargetBuildError { failure_kind }
}

#[cfg(test)]
#[path = "dependency_bridge_public_type_errors_tests.rs"]
mod tests;
