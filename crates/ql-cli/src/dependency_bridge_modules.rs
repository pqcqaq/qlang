use std::fs;
use std::path::{Path, PathBuf};

use ql_ast::Module;
use ql_parser::parse_source;
use ql_project::{
    BuildTargetKind, WorkspaceBuildTargets, default_interface_path, load_interface_artifact,
    load_reference_manifests, package_name,
};

use crate::build_plan::{
    PrepareProjectTargetBuildError, report_project_build_dependency_error,
    target_prep_dependency_interface_failure, target_prep_dependency_manifest_failure,
    target_prep_dependency_source_parse_failure, target_prep_dependency_source_read_failure,
};
use crate::dependency_bridge_reporting::{
    report_dependency_interface_load_failure, report_dependency_source_parse_failure,
    report_dependency_source_read_failure, report_package_under_test_source_parse_failure,
    report_package_under_test_source_read_failure,
};
use crate::dependency_generic_bridge;

const DEPENDENCY_GENERIC_SPECIALIZATION_CONTEXT: &str = "imported generic helper specializations";

pub(crate) struct PackageBridgeModule {
    pub(crate) source: String,
    pub(crate) module: Module,
}

pub(crate) struct DependencyGenericSpecializationSourceModule {
    pub(crate) module_import_path: Vec<String>,
    pub(crate) source: String,
    pub(crate) module: Module,
}

pub(crate) fn dependency_generic_specialization_module_refs(
    modules: &[DependencyGenericSpecializationSourceModule],
) -> Vec<dependency_generic_bridge::SpecializationModule<'_>> {
    modules
        .iter()
        .map(|module| dependency_generic_bridge::SpecializationModule {
            module_import_path: &module.module_import_path,
            contents: &module.source,
            module: &module.module,
        })
        .collect()
}

enum DependencyGenericSpecializationModuleLoadError {
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

pub(crate) fn dependency_interface_module_import_paths(
    dependency_package: &str,
    modules: &[ql_project::InterfaceModule],
) -> std::collections::BTreeSet<Vec<String>> {
    modules
        .iter()
        .map(|module| dependency_interface_module_import_path(dependency_package, &module.syntax))
        .collect()
}

pub(crate) fn dependency_interface_module_import_path(
    dependency_package: &str,
    module: &Module,
) -> Vec<String> {
    module
        .package
        .as_ref()
        .map(|package| package.path.segments.clone())
        .unwrap_or_else(|| dependency_package.split('.').map(str::to_owned).collect())
}

pub(crate) fn dependency_module_source_path(
    dependency_manifest_path: &Path,
    source_path: &str,
) -> PathBuf {
    dependency_manifest_path
        .parent()
        .unwrap_or(Path::new("."))
        .join(source_path)
}

pub(crate) fn package_under_test_bridge_modules(
    command_label: &str,
    member: &WorkspaceBuildTargets,
    report_failure: bool,
) -> Result<Vec<PackageBridgeModule>, u8> {
    let mut modules = Vec::new();
    for target in &member.targets {
        if target.kind != BuildTargetKind::Library {
            continue;
        }
        let source = fs::read_to_string(&target.path).map_err(|error| {
            if report_failure {
                report_package_under_test_source_read_failure(command_label, &target.path, error);
            }
            1
        })?;
        let module = parse_source(&source).map_err(|_| {
            if report_failure {
                report_package_under_test_source_parse_failure(command_label, &target.path);
            }
            1
        })?;
        modules.push(PackageBridgeModule { source, module });
    }
    Ok(modules)
}

pub(crate) fn dependency_generic_specialization_modules(
    command_label: &str,
    owner_manifest: &ql_project::ProjectManifest,
    report_failure: bool,
) -> Result<Vec<DependencyGenericSpecializationSourceModule>, u8> {
    let dependencies = load_reference_manifests(owner_manifest).map_err(|error| {
        if report_failure {
            report_project_build_dependency_error(
                command_label,
                Some(&owner_manifest.manifest_path),
                &error,
            );
        }
        1
    })?;
    load_dependency_generic_specialization_modules_from_dependencies(&dependencies).map_err(
        |error| {
            if report_failure {
                report_dependency_generic_specialization_module_load_error(
                    command_label,
                    owner_manifest,
                    &error,
                );
            }
            1
        },
    )
}

pub(crate) fn dependency_generic_specialization_modules_quiet(
    owner_manifest: &ql_project::ProjectManifest,
) -> Result<Vec<DependencyGenericSpecializationSourceModule>, PrepareProjectTargetBuildError> {
    let dependencies = load_reference_manifests(owner_manifest)
        .map_err(|error| target_prep_dependency_manifest_failure(None, &error))?;
    load_dependency_generic_specialization_modules_from_dependencies(&dependencies)
        .map_err(dependency_generic_specialization_module_load_error_to_target_prep_error)
}

fn load_dependency_generic_specialization_modules_from_dependencies(
    dependencies: &[ql_project::ProjectManifest],
) -> Result<
    Vec<DependencyGenericSpecializationSourceModule>,
    DependencyGenericSpecializationModuleLoadError,
> {
    let mut modules = Vec::new();
    for dependency in dependencies {
        let dependency_package = package_name(dependency)
            .map(str::to_owned)
            .map_err(|error| {
                DependencyGenericSpecializationModuleLoadError::DependencyManifest {
                    manifest_path: dependency.manifest_path.clone(),
                    error,
                }
            })?;
        let interface_path = default_interface_path(dependency).map_err(|error| {
            DependencyGenericSpecializationModuleLoadError::DependencyManifest {
                manifest_path: dependency.manifest_path.clone(),
                error,
            }
        })?;
        let artifact = load_interface_artifact(&interface_path).map_err(|error| {
            DependencyGenericSpecializationModuleLoadError::DependencyInterface {
                dependency_manifest_path: dependency.manifest_path.clone(),
                dependency_package: dependency_package.clone(),
                interface_path: interface_path.clone(),
                detail: error.to_string(),
            }
        })?;
        for module in &artifact.modules {
            let dependency_source_path =
                dependency_module_source_path(&dependency.manifest_path, &module.source_path);
            let source = fs::read_to_string(&dependency_source_path).map_err(|error| {
                DependencyGenericSpecializationModuleLoadError::DependencySourceRead {
                    dependency_manifest_path: dependency.manifest_path.clone(),
                    dependency_package: dependency_package.clone(),
                    source_path: dependency_source_path.clone(),
                    detail: error.to_string(),
                }
            })?;
            let parsed = parse_source(&source).map_err(|_| {
                DependencyGenericSpecializationModuleLoadError::DependencySourceParse {
                    dependency_manifest_path: dependency.manifest_path.clone(),
                    dependency_package: dependency_package.clone(),
                    source_path: dependency_source_path.clone(),
                }
            })?;
            let module_import_path =
                dependency_interface_module_import_path(&dependency_package, &parsed);
            modules.push(DependencyGenericSpecializationSourceModule {
                module_import_path,
                source,
                module: parsed,
            });
        }
    }
    Ok(modules)
}

fn report_dependency_generic_specialization_module_load_error(
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

fn dependency_generic_specialization_module_load_error_to_target_prep_error(
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
mod tests {
    use super::*;

    #[test]
    fn dependency_generic_specialization_quiet_mapping_reuses_target_prep_helpers() {
        let interface_failure =
            dependency_generic_specialization_module_load_error_to_target_prep_error(
                DependencyGenericSpecializationModuleLoadError::DependencyInterface {
                    dependency_manifest_path: PathBuf::from("workspace/dep/qlang.toml"),
                    dependency_package: "dep".to_owned(),
                    interface_path: PathBuf::from("workspace/dep/dep.qi"),
                    detail: "missing interface".to_owned(),
                },
            );
        let source_read_failure =
            dependency_generic_specialization_module_load_error_to_target_prep_error(
                DependencyGenericSpecializationModuleLoadError::DependencySourceRead {
                    dependency_manifest_path: PathBuf::from("workspace/dep/qlang.toml"),
                    dependency_package: "dep".to_owned(),
                    source_path: PathBuf::from("workspace/dep/src/lib.ql"),
                    detail: "access denied".to_owned(),
                },
            );
        let source_parse_failure =
            dependency_generic_specialization_module_load_error_to_target_prep_error(
                DependencyGenericSpecializationModuleLoadError::DependencySourceParse {
                    dependency_manifest_path: PathBuf::from("workspace/dep/qlang.toml"),
                    dependency_package: "dep".to_owned(),
                    source_path: PathBuf::from("workspace/dep/src/lib.ql"),
                },
            );

        match interface_failure.failure_kind {
            crate::build_plan::PrepareProjectTargetBuildFailureKind::DependencyInterface {
                message,
                ..
            } => assert_eq!(
                message,
                "failed to load referenced package interface `workspace/dep/dep.qi`: missing interface"
            ),
            _ => panic!("expected dependency interface failure"),
        }
        match source_read_failure.failure_kind {
            crate::build_plan::PrepareProjectTargetBuildFailureKind::DependencySource {
                message,
                ..
            } => assert_eq!(
                message,
                "failed to access dependency source `workspace/dep/src/lib.ql`: access denied"
            ),
            _ => panic!("expected dependency source read failure"),
        }
        match source_parse_failure.failure_kind {
            crate::build_plan::PrepareProjectTargetBuildFailureKind::DependencySource {
                message,
                ..
            } => assert_eq!(
                message,
                "failed to parse dependency source `workspace/dep/src/lib.ql` while preparing imported generic helper specializations"
            ),
            _ => panic!("expected dependency source parse failure"),
        }
    }
}
