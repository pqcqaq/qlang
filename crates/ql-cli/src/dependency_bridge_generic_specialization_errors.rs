use std::path::PathBuf;

use crate::build_plan::{
    PrepareProjectTargetBuildError, report_project_build_dependency_error,
    target_prep_dependency_interface_failure, target_prep_dependency_manifest_failure,
    target_prep_dependency_source_parse_failure, target_prep_dependency_source_read_failure,
};
use crate::dependency_bridge_reporting::{
    report_dependency_interface_load_failure, report_dependency_source_parse_failure,
    report_dependency_source_read_failure,
};

const DEPENDENCY_GENERIC_SPECIALIZATION_CONTEXT: &str = "imported generic helper specializations";

pub(crate) enum DependencyGenericSpecializationModuleLoadError {
    DependencyManifest {
        manifest_path: PathBuf,
        error: ql_project::ProjectError,
    },
    DependencyInterface {
        dependency_manifest_path: PathBuf,
        dependency_package: String,
        interface_path: PathBuf,
        detail: String,
    },
    DependencySourceRead {
        dependency_manifest_path: PathBuf,
        dependency_package: String,
        source_path: PathBuf,
        detail: String,
    },
    DependencySourceParse {
        dependency_manifest_path: PathBuf,
        dependency_package: String,
        source_path: PathBuf,
    },
}

pub(crate) fn report_dependency_generic_specialization_module_load_error(
    command_label: &str,
    owner_manifest: &ql_project::ProjectManifest,
    error: &DependencyGenericSpecializationModuleLoadError,
) {
    match error {
        DependencyGenericSpecializationModuleLoadError::DependencyManifest {
            manifest_path,
            error,
        } => report_project_build_dependency_error(command_label, Some(manifest_path), error),
        DependencyGenericSpecializationModuleLoadError::DependencyInterface {
            interface_path,
            detail,
            ..
        } => report_dependency_interface_load_failure(
            command_label,
            &owner_manifest.manifest_path,
            DEPENDENCY_GENERIC_SPECIALIZATION_CONTEXT,
            interface_path,
            detail,
        ),
        DependencyGenericSpecializationModuleLoadError::DependencySourceRead {
            source_path,
            detail,
            ..
        } => report_dependency_source_read_failure(
            command_label,
            &owner_manifest.manifest_path,
            DEPENDENCY_GENERIC_SPECIALIZATION_CONTEXT,
            source_path,
            detail,
        ),
        DependencyGenericSpecializationModuleLoadError::DependencySourceParse {
            dependency_package,
            source_path,
            ..
        } => {
            report_dependency_source_parse_failure(
                command_label,
                dependency_package,
                source_path,
                DEPENDENCY_GENERIC_SPECIALIZATION_CONTEXT,
            );
        }
    }
}

pub(crate) fn dependency_generic_specialization_module_load_error_to_target_prep_error(
    error: DependencyGenericSpecializationModuleLoadError,
) -> PrepareProjectTargetBuildError {
    match error {
        DependencyGenericSpecializationModuleLoadError::DependencyManifest {
            manifest_path,
            error,
        } => target_prep_dependency_manifest_failure(Some(&manifest_path), &error),
        DependencyGenericSpecializationModuleLoadError::DependencyInterface {
            dependency_manifest_path,
            dependency_package,
            interface_path,
            detail,
        } => target_prep_dependency_interface_failure(
            &dependency_manifest_path,
            &dependency_package,
            &interface_path,
            detail,
        ),
        DependencyGenericSpecializationModuleLoadError::DependencySourceRead {
            dependency_manifest_path,
            dependency_package,
            source_path,
            detail,
        } => target_prep_dependency_source_read_failure(
            &dependency_manifest_path,
            &dependency_package,
            &source_path,
            detail,
        ),
        DependencyGenericSpecializationModuleLoadError::DependencySourceParse {
            dependency_manifest_path,
            dependency_package,
            source_path,
        } => target_prep_dependency_source_parse_failure(
            &dependency_manifest_path,
            &dependency_package,
            &source_path,
            DEPENDENCY_GENERIC_SPECIALIZATION_CONTEXT,
        ),
    }
}

#[cfg(test)]
#[path = "dependency_bridge_generic_specialization_errors_tests.rs"]
mod tests;
