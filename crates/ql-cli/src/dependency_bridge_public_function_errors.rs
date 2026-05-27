use std::path::Path;

use crate::build_plan::{PrepareProjectTargetBuildError, PrepareProjectTargetBuildFailureKind};
use crate::dependency_bridge_names::DependencyExternOwner;
use crate::dependency_bridge_reporting::{
    report_direct_dependency_local_conflict, report_direct_dependency_symbol_conflict,
    report_direct_dependency_unsupported_generic_function,
    report_package_under_test_local_conflict, report_package_under_test_symbol_conflict,
    report_package_under_test_unsupported_generic_function,
};

pub(crate) enum DependencyPublicFunctionForwarderError {
    DependencyConflict {
        symbol: String,
        owner: DependencyExternOwner,
    },
    LocalConflict {
        symbol: String,
    },
    UnsupportedGeneric {
        symbol: String,
    },
}

pub(crate) fn report_direct_dependency_function_forwarder_error(
    command_label: &str,
    dependency_package: &str,
    error: DependencyPublicFunctionForwarderError,
) {
    match error {
        DependencyPublicFunctionForwarderError::DependencyConflict { symbol, owner } => {
            report_direct_dependency_symbol_conflict(
                command_label,
                "public function",
                &symbol,
                &owner.package_name,
                dependency_package,
                "keep direct dependency public function names unique until package-qualified dependency call lowering lands",
            );
        }
        DependencyPublicFunctionForwarderError::LocalConflict { symbol } => {
            report_direct_dependency_local_conflict(
                command_label,
                "public function",
                &symbol,
                dependency_package,
                "rename the local top-level item or avoid importing a direct dependency public function with the same original symbol name",
            );
        }
        DependencyPublicFunctionForwarderError::UnsupportedGeneric { symbol } => {
            report_direct_dependency_unsupported_generic_function(
                command_label,
                &symbol,
                dependency_package,
            );
        }
    }
}

pub(crate) fn report_package_under_test_function_forwarder_error(
    command_label: &str,
    package_name: &str,
    error: DependencyPublicFunctionForwarderError,
) {
    match error {
        DependencyPublicFunctionForwarderError::DependencyConflict { symbol, owner } => {
            report_package_under_test_symbol_conflict(
                command_label,
                "public function",
                &symbol,
                &owner.package_name,
                package_name,
            );
        }
        DependencyPublicFunctionForwarderError::LocalConflict { symbol } => {
            report_package_under_test_local_conflict(
                command_label,
                "public function",
                &symbol,
                "rename the local top-level item or avoid importing the package-under-test public function with the same original symbol name",
            );
        }
        DependencyPublicFunctionForwarderError::UnsupportedGeneric { symbol } => {
            report_package_under_test_unsupported_generic_function(command_label, &symbol);
        }
    }
}

pub(crate) fn dependency_function_forwarder_target_prep_error(
    error: DependencyPublicFunctionForwarderError,
    dependency_package: &str,
    dependency_manifest_path: &Path,
) -> PrepareProjectTargetBuildError {
    let failure_kind = match error {
        DependencyPublicFunctionForwarderError::DependencyConflict { symbol, owner } => {
            PrepareProjectTargetBuildFailureKind::DependencyFunctionConflict {
                symbol,
                first_package: owner.package_name,
                first_manifest_path: owner.manifest_path,
                conflicting_package: dependency_package.to_owned(),
                conflicting_manifest_path: dependency_manifest_path.to_path_buf(),
            }
        }
        DependencyPublicFunctionForwarderError::LocalConflict { symbol } => {
            PrepareProjectTargetBuildFailureKind::DependencyFunctionLocalConflict {
                symbol,
                dependency_package: dependency_package.to_owned(),
                dependency_manifest_path: dependency_manifest_path.to_path_buf(),
            }
        }
        DependencyPublicFunctionForwarderError::UnsupportedGeneric { symbol } => {
            PrepareProjectTargetBuildFailureKind::DependencyFunctionUnsupportedGeneric {
                symbol,
                dependency_package: dependency_package.to_owned(),
                dependency_manifest_path: dependency_manifest_path.to_path_buf(),
            }
        }
    };
    PrepareProjectTargetBuildError { failure_kind }
}

#[cfg(test)]
#[path = "dependency_bridge_public_function_errors_tests.rs"]
mod tests;
