use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use ql_parser::parse_source;
use ql_project::{
    default_interface_path, load_interface_artifact, load_project_manifest,
    load_reference_manifests, package_name,
};

use crate::build_plan::{
    PrepareProjectTargetBuildError, report_project_build_dependency_error,
    target_prep_dependency_interface_failure, target_prep_dependency_manifest_failure,
    target_prep_dependency_source_parse_failure, target_prep_dependency_source_read_failure,
};
use crate::dependency_bridge_imports::collect_top_level_definition_names;
use crate::dependency_bridge_modules::{
    dependency_generic_specialization_module_refs, dependency_generic_specialization_modules,
    dependency_generic_specialization_modules_quiet,
};
use crate::dependency_bridge_names::DependencyExternOwner;
use crate::dependency_bridge_public_function_errors::{
    dependency_function_forwarder_target_prep_error,
    report_direct_dependency_function_forwarder_error,
};
use crate::dependency_bridge_public_function_forwarders::collect_dependency_public_function_forwarders_from_modules;
use crate::dependency_bridge_reporting::{
    report_dependency_interface_load_failure, report_dependency_source_parse_failure,
    report_dependency_source_read_failure,
};
use crate::dependency_generic_bridge;
use crate::project_manifest_paths::reference_manifest_path;

#[derive(Default)]
pub(crate) struct RenderedDependencyPublicFunctionForwarders {
    pub(crate) forwarders: String,
    pub(crate) source_rewrites: Vec<dependency_generic_bridge::SourceRewrite>,
    pub(crate) required_types_by_module_path: BTreeMap<Vec<String>, BTreeSet<String>>,
}

pub(crate) fn render_direct_dependency_public_function_forwarders(
    command_label: &str,
    manifest_path: &Path,
    source: &str,
    required_functions_by_module_path: &BTreeMap<Vec<String>, BTreeSet<String>>,
    report_failure: bool,
) -> Result<RenderedDependencyPublicFunctionForwarders, u8> {
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
        Err(_) => return Ok(RenderedDependencyPublicFunctionForwarders::default()),
    };
    let occupied_root_names = collect_top_level_definition_names(&root_source_module);

    let mut forwarders = Vec::new();
    let mut source_rewrites = Vec::new();
    let mut owners_by_symbol = BTreeMap::<String, DependencyExternOwner>::new();
    let mut required_types_by_module_path = BTreeMap::<Vec<String>, BTreeSet<String>>::new();
    let mut rendered_specializations = BTreeSet::new();

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
                    "dependency public function wrappers",
                    &interface_path,
                    error,
                );
            }
            1
        })?;
        let specialization_modules =
            dependency_generic_specialization_modules(command_label, &dependency, report_failure)?;
        let specialization_modules =
            dependency_generic_specialization_module_refs(&specialization_modules);
        collect_dependency_public_function_forwarders_from_modules(
            &dependency_package,
            &dependency.manifest_path,
            &artifact.modules,
            &root_source_module,
            required_functions_by_module_path,
            &occupied_root_names,
            &specialization_modules,
            &mut required_types_by_module_path,
            &mut owners_by_symbol,
            &mut forwarders,
            &mut source_rewrites,
            &mut rendered_specializations,
            |dependency_source_path| {
                fs::read_to_string(dependency_source_path).map_err(|error| {
                    if report_failure {
                        report_dependency_source_read_failure(
                            command_label,
                            manifest_path,
                            "dependency public function wrappers",
                            dependency_source_path,
                            error,
                        );
                    }
                    1
                })
            },
            |dependency_source_path, dependency_source| {
                parse_source(dependency_source).map_err(|_| {
                    if report_failure {
                        report_dependency_source_parse_failure(
                            command_label,
                            &dependency_package,
                            dependency_source_path,
                            "public function wrappers",
                        );
                    }
                    1
                })
            },
            |error| {
                if report_failure {
                    report_direct_dependency_function_forwarder_error(
                        command_label,
                        &dependency_package,
                        error,
                    );
                }
                1
            },
        )?;
    }

    Ok(RenderedDependencyPublicFunctionForwarders {
        forwarders: forwarders.join("\n\n"),
        source_rewrites,
        required_types_by_module_path,
    })
}

pub(crate) fn render_direct_dependency_public_function_forwarders_quiet(
    manifest_path: &Path,
    source: &str,
    required_functions_by_module_path: &BTreeMap<Vec<String>, BTreeSet<String>>,
) -> Result<RenderedDependencyPublicFunctionForwarders, PrepareProjectTargetBuildError> {
    let manifest = load_project_manifest(manifest_path)
        .map_err(|error| target_prep_dependency_manifest_failure(None, &error))?;
    let manifest_dir = manifest.manifest_path.parent().unwrap_or(Path::new("."));
    let root_source_module = match parse_source(source) {
        Ok(module) => module,
        Err(_) => return Ok(RenderedDependencyPublicFunctionForwarders::default()),
    };
    let occupied_root_names = collect_top_level_definition_names(&root_source_module);
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

    let mut forwarders = Vec::new();
    let mut source_rewrites = Vec::new();
    let mut owners_by_symbol = BTreeMap::<String, DependencyExternOwner>::new();
    let mut required_types_by_module_path = BTreeMap::<Vec<String>, BTreeSet<String>>::new();
    let mut rendered_specializations = BTreeSet::new();

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
        let specialization_modules =
            dependency_generic_specialization_modules_quiet(&dependency_manifest)?;
        let specialization_modules =
            dependency_generic_specialization_module_refs(&specialization_modules);
        collect_dependency_public_function_forwarders_from_modules(
            &dependency_package,
            &dependency_manifest.manifest_path,
            &artifact.modules,
            &root_source_module,
            required_functions_by_module_path,
            &occupied_root_names,
            &specialization_modules,
            &mut required_types_by_module_path,
            &mut owners_by_symbol,
            &mut forwarders,
            &mut source_rewrites,
            &mut rendered_specializations,
            |dependency_source_path| {
                fs::read_to_string(dependency_source_path).map_err(|error| {
                    target_prep_dependency_source_read_failure(
                        &dependency_manifest.manifest_path,
                        &dependency_package,
                        dependency_source_path,
                        error,
                    )
                })
            },
            |dependency_source_path, dependency_source| {
                parse_source(dependency_source).map_err(|_| {
                    target_prep_dependency_source_parse_failure(
                        &dependency_manifest.manifest_path,
                        &dependency_package,
                        dependency_source_path,
                        "public function wrappers",
                    )
                })
            },
            |error| {
                dependency_function_forwarder_target_prep_error(
                    error,
                    &dependency_package,
                    &dependency_manifest.manifest_path,
                )
            },
        )?;
    }

    Ok(RenderedDependencyPublicFunctionForwarders {
        forwarders: forwarders.join("\n\n"),
        source_rewrites,
        required_types_by_module_path,
    })
}
