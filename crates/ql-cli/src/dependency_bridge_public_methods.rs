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
use crate::dependency_bridge_public_method_forwarders::collect_dependency_public_method_forwarders_from_modules;
use crate::dependency_bridge_reporting::{
    report_dependency_interface_load_failure, report_dependency_source_parse_failure,
    report_dependency_source_read_failure,
};
use crate::project_manifest_paths::reference_manifest_path;

#[derive(Default)]
pub(crate) struct RenderedDependencyPublicMethodForwarders {
    pub(crate) forwarders: String,
    pub(crate) required_types_by_module_path: BTreeMap<Vec<String>, BTreeSet<String>>,
}

pub(crate) fn render_direct_dependency_public_method_forwarders(
    command_label: &str,
    manifest_path: &Path,
    required_types_by_module_path: &BTreeMap<Vec<String>, BTreeSet<String>>,
    report_failure: bool,
) -> Result<RenderedDependencyPublicMethodForwarders, u8> {
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

    let mut forwarders = Vec::new();
    let mut discovered_required_types = BTreeMap::<Vec<String>, BTreeSet<String>>::new();

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
                    "dependency public method bridges",
                    &interface_path,
                    error,
                );
            }
            1
        })?;

        collect_dependency_public_method_forwarders_from_modules(
            &dependency_package,
            &dependency.manifest_path,
            &artifact.modules,
            required_types_by_module_path,
            &mut discovered_required_types,
            &mut forwarders,
            |dependency_source_path| {
                fs::read_to_string(dependency_source_path).map_err(|error| {
                    if report_failure {
                        report_dependency_source_read_failure(
                            command_label,
                            manifest_path,
                            "dependency public method bridges",
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
                            "public method bridges",
                        );
                    }
                    1
                })
            },
        )?;
    }

    Ok(RenderedDependencyPublicMethodForwarders {
        forwarders: forwarders.join("\n\n"),
        required_types_by_module_path: discovered_required_types,
    })
}

pub(crate) fn render_direct_dependency_public_method_forwarders_quiet(
    manifest_path: &Path,
    required_types_by_module_path: &BTreeMap<Vec<String>, BTreeSet<String>>,
) -> Result<RenderedDependencyPublicMethodForwarders, PrepareProjectTargetBuildError> {
    let manifest = load_project_manifest(manifest_path)
        .map_err(|error| target_prep_dependency_manifest_failure(None, &error))?;
    let manifest_dir = manifest.manifest_path.parent().unwrap_or(Path::new("."));
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
    let mut discovered_required_types = BTreeMap::<Vec<String>, BTreeSet<String>>::new();

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

        collect_dependency_public_method_forwarders_from_modules(
            &dependency_package,
            &dependency_manifest.manifest_path,
            &artifact.modules,
            required_types_by_module_path,
            &mut discovered_required_types,
            &mut forwarders,
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
                        "public method bridges",
                    )
                })
            },
        )?;
    }

    Ok(RenderedDependencyPublicMethodForwarders {
        forwarders: forwarders.join("\n\n"),
        required_types_by_module_path: discovered_required_types,
    })
}
