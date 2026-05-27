use std::collections::BTreeMap;
use std::path::Path;

use ql_parser::parse_source;
use ql_project::{
    default_interface_path, load_interface_artifact, load_project_manifest,
    load_reference_manifests, package_name,
};

use crate::build_plan::{
    PrepareProjectTargetBuildError, report_project_build_dependency_error,
    target_prep_dependency_interface_failure, target_prep_dependency_manifest_failure,
};
use crate::dependency_bridge_extern_declarations::collect_dependency_module_extern_declarations;
use crate::dependency_bridge_extern_errors::{
    dependency_extern_bridge_target_prep_error, report_direct_dependency_extern_bridge_error,
};
use crate::dependency_bridge_names::DependencyExternOwner;
use crate::dependency_bridge_reporting::report_dependency_interface_load_failure;
use crate::dependency_bridge_source_modules::collect_dependency_interface_module_bridge_items;
use crate::project_manifest_paths::reference_manifest_path;

pub(crate) fn render_direct_dependency_extern_declarations(
    command_label: &str,
    manifest_path: &Path,
    source: &str,
    report_failure: bool,
) -> Result<String, u8> {
    let manifest = load_project_manifest(manifest_path).map_err(|error| {
        if report_failure {
            report_project_build_dependency_error(command_label, None, &error);
        }
        1
    })?;
    let direct_dependencies = load_reference_manifests(&manifest).map_err(|error| {
        if report_failure {
            report_project_build_dependency_error(command_label, Some(manifest_path), &error);
        }
        1
    })?;
    let root_source_module = match parse_source(source) {
        Ok(module) => module,
        Err(_) => return Ok(String::new()),
    };

    let mut declarations = Vec::new();
    let mut owners_by_symbol = BTreeMap::<String, DependencyExternOwner>::new();

    for dependency in direct_dependencies {
        let dependency_package = match package_name(&dependency) {
            Ok(name) => name.to_owned(),
            Err(error) => {
                if report_failure {
                    report_project_build_dependency_error(
                        command_label,
                        Some(manifest_path),
                        &error,
                    );
                }
                return Err(1);
            }
        };
        let interface_path = default_interface_path(&dependency).map_err(|error| {
            if report_failure {
                report_project_build_dependency_error(command_label, Some(manifest_path), &error);
            }
            1
        })?;
        let artifact = load_interface_artifact(&interface_path).map_err(|error| {
            if report_failure {
                report_dependency_interface_load_failure(
                    command_label,
                    manifest_path,
                    "dependency extern declarations",
                    &interface_path,
                    error,
                );
            }
            1
        })?;
        collect_dependency_interface_module_bridge_items(
            &dependency_package,
            &artifact.modules,
            &root_source_module,
            |module, imported_externs, _module_import_path| {
                collect_dependency_module_extern_declarations(
                    &dependency_package,
                    &dependency.manifest_path,
                    &module.syntax,
                    &module.contents,
                    Some(imported_externs),
                    &mut owners_by_symbol,
                    &mut declarations,
                )
                .map_err(|error| {
                    if report_failure {
                        report_direct_dependency_extern_bridge_error(
                            command_label,
                            &dependency_package,
                            error,
                        );
                    }
                    1
                })
            },
        )?;
    }

    Ok(declarations.join("\n\n"))
}

pub(crate) fn render_direct_dependency_extern_declarations_quiet(
    manifest_path: &Path,
    source: &str,
) -> Result<String, PrepareProjectTargetBuildError> {
    let manifest = load_project_manifest(manifest_path)
        .map_err(|error| target_prep_dependency_manifest_failure(None, &error))?;
    let manifest_dir = manifest.manifest_path.parent().unwrap_or(Path::new("."));
    let root_source_module = match parse_source(source) {
        Ok(module) => module,
        Err(_) => return Ok(String::new()),
    };
    let direct_dependencies = manifest
        .references
        .packages
        .iter()
        .map(|reference| {
            let reference_manifest_path = reference_manifest_path(&manifest, reference);
            let dependency_manifest = load_project_manifest(&manifest_dir.join(reference))
                .map_err(|error| {
                    target_prep_dependency_manifest_failure(Some(&reference_manifest_path), &error)
                })?;
            Ok::<_, PrepareProjectTargetBuildError>(dependency_manifest)
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut declarations = Vec::new();
    let mut owners_by_symbol = BTreeMap::<String, DependencyExternOwner>::new();

    for dependency_manifest in direct_dependencies {
        let dependency_package = package_name(&dependency_manifest)
            .map(str::to_owned)
            .map_err(|error| {
                target_prep_dependency_manifest_failure(
                    Some(&dependency_manifest.manifest_path),
                    &error,
                )
            })?;
        let interface_path = default_interface_path(&dependency_manifest).map_err(|error| {
            target_prep_dependency_manifest_failure(
                Some(&dependency_manifest.manifest_path),
                &error,
            )
        })?;
        let artifact = load_interface_artifact(&interface_path).map_err(|error| {
            target_prep_dependency_interface_failure(
                &dependency_manifest.manifest_path,
                &dependency_package,
                &interface_path,
                error,
            )
        })?;
        collect_dependency_interface_module_bridge_items(
            &dependency_package,
            &artifact.modules,
            &root_source_module,
            |module, imported_externs, _module_import_path| {
                collect_dependency_module_extern_declarations(
                    &dependency_package,
                    &dependency_manifest.manifest_path,
                    &module.syntax,
                    &module.contents,
                    Some(imported_externs),
                    &mut owners_by_symbol,
                    &mut declarations,
                )
                .map_err(|error| {
                    dependency_extern_bridge_target_prep_error(
                        error,
                        &dependency_package,
                        &dependency_manifest.manifest_path,
                    )
                })
            },
        )?;
    }

    Ok(declarations.join("\n\n"))
}
