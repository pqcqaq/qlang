use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use ql_parser::parse_source;
use ql_project::{load_project_manifest, load_reference_manifests};

use crate::build_plan::{
    PrepareProjectTargetBuildError, report_project_build_dependency_error,
    target_prep_dependency_interface_failure, target_prep_dependency_manifest_failure,
    target_prep_dependency_source_parse_failure, target_prep_dependency_source_read_failure,
};
use crate::dependency_bridge_imports::collect_top_level_definition_names;
use crate::dependency_bridge_names::DependencyExternOwner;
use crate::dependency_bridge_public_value_declarations::collect_dependency_module_public_value_declarations;
use crate::dependency_bridge_public_value_errors::{
    dependency_value_bridge_target_prep_error, report_direct_dependency_value_bridge_error,
};
use crate::dependency_bridge_reporting::{
    report_dependency_interface_load_failure, report_dependency_source_parse_failure,
    report_dependency_source_read_failure,
};
use crate::dependency_bridge_source_modules::collect_direct_dependency_source_module_bridge_items;
use crate::project_manifest_paths::reference_manifest_path;

#[derive(Default)]
pub(crate) struct RenderedDependencyPublicValueDeclarations {
    pub(crate) declarations: String,
    pub(crate) required_functions_by_module_path: BTreeMap<Vec<String>, BTreeSet<String>>,
    pub(crate) required_types_by_module_path: BTreeMap<Vec<String>, BTreeSet<String>>,
}

pub(crate) fn render_direct_dependency_public_value_declarations(
    command_label: &str,
    manifest_path: &Path,
    source: &str,
    report_failure: bool,
) -> Result<RenderedDependencyPublicValueDeclarations, u8> {
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
        Err(_) => return Ok(RenderedDependencyPublicValueDeclarations::default()),
    };
    let occupied_root_names = collect_top_level_definition_names(&root_source_module);

    let mut declarations = Vec::new();
    let mut owners_by_symbol = BTreeMap::<String, DependencyExternOwner>::new();
    let mut required_functions_by_module_path = BTreeMap::<Vec<String>, BTreeSet<String>>::new();
    let mut required_types_by_module_path = BTreeMap::<Vec<String>, BTreeSet<String>>::new();

    collect_direct_dependency_source_module_bridge_items(
        &direct_dependencies,
        &root_source_module,
        |_dependency, error| {
            if report_failure {
                report_project_build_dependency_error(command_label, Some(manifest_path), error);
            }
            1
        },
        |_dependency, error| {
            if report_failure {
                report_project_build_dependency_error(command_label, Some(manifest_path), error);
            }
            1
        },
        |_dependency, _dependency_package, interface_path, error| {
            if report_failure {
                report_dependency_interface_load_failure(
                    command_label,
                    manifest_path,
                    "dependency public value bridges",
                    interface_path,
                    error,
                );
            }
            1
        },
        |_dependency, _dependency_package, dependency_source_path| {
            fs::read_to_string(dependency_source_path).map_err(|error| {
                if report_failure {
                    report_dependency_source_read_failure(
                        command_label,
                        manifest_path,
                        "dependency public value bridges",
                        dependency_source_path,
                        error,
                    );
                }
                1
            })
        },
        |_dependency, dependency_package, dependency_source_path, dependency_source| {
            parse_source(dependency_source).map_err(|_| {
                if report_failure {
                    report_dependency_source_parse_failure(
                        command_label,
                        dependency_package,
                        dependency_source_path,
                        "public value bridges",
                    );
                }
                1
            })
        },
        |dependency,
         dependency_package,
         source_module,
         dependency_source,
         imported_externs,
         _module_import_path| {
            collect_dependency_module_public_value_declarations(
                dependency_package,
                &dependency.manifest_path,
                source_module,
                dependency_source,
                Some(imported_externs),
                &occupied_root_names,
                &mut required_functions_by_module_path,
                &mut required_types_by_module_path,
                &mut owners_by_symbol,
                &mut declarations,
            )
            .map_err(|error| {
                if report_failure {
                    report_direct_dependency_value_bridge_error(
                        command_label,
                        dependency_package,
                        error,
                    );
                }
                1
            })
        },
    )?;

    Ok(RenderedDependencyPublicValueDeclarations {
        declarations: declarations.join("\n\n"),
        required_functions_by_module_path,
        required_types_by_module_path,
    })
}

pub(crate) fn render_direct_dependency_public_value_declarations_quiet(
    manifest_path: &Path,
    source: &str,
) -> Result<RenderedDependencyPublicValueDeclarations, PrepareProjectTargetBuildError> {
    let manifest = load_project_manifest(manifest_path)
        .map_err(|error| target_prep_dependency_manifest_failure(None, &error))?;
    let manifest_dir = manifest.manifest_path.parent().unwrap_or(Path::new("."));
    let root_source_module = match parse_source(source) {
        Ok(module) => module,
        Err(_) => return Ok(RenderedDependencyPublicValueDeclarations::default()),
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

    let mut declarations = Vec::new();
    let mut owners_by_symbol = BTreeMap::<String, DependencyExternOwner>::new();
    let mut required_functions_by_module_path = BTreeMap::<Vec<String>, BTreeSet<String>>::new();
    let mut required_types_by_module_path = BTreeMap::<Vec<String>, BTreeSet<String>>::new();

    collect_direct_dependency_source_module_bridge_items(
        &direct_dependencies,
        &root_source_module,
        |dependency_manifest, error| {
            target_prep_dependency_manifest_failure(Some(&dependency_manifest.manifest_path), error)
        },
        |dependency_manifest, error| {
            target_prep_dependency_manifest_failure(Some(&dependency_manifest.manifest_path), error)
        },
        |dependency_manifest, dependency_package, interface_path, error| {
            target_prep_dependency_interface_failure(
                &dependency_manifest.manifest_path,
                dependency_package,
                interface_path,
                error,
            )
        },
        |dependency_manifest, dependency_package, dependency_source_path| {
            fs::read_to_string(dependency_source_path).map_err(|error| {
                target_prep_dependency_source_read_failure(
                    &dependency_manifest.manifest_path,
                    dependency_package,
                    dependency_source_path,
                    error,
                )
            })
        },
        |dependency_manifest, dependency_package, dependency_source_path, dependency_source| {
            parse_source(dependency_source).map_err(|_| {
                target_prep_dependency_source_parse_failure(
                    &dependency_manifest.manifest_path,
                    dependency_package,
                    dependency_source_path,
                    "public value bridges",
                )
            })
        },
        |dependency_manifest,
         dependency_package,
         source_module,
         dependency_source,
         imported_externs,
         _module_import_path| {
            collect_dependency_module_public_value_declarations(
                dependency_package,
                &dependency_manifest.manifest_path,
                source_module,
                dependency_source,
                Some(imported_externs),
                &occupied_root_names,
                &mut required_functions_by_module_path,
                &mut required_types_by_module_path,
                &mut owners_by_symbol,
                &mut declarations,
            )
            .map_err(|error| {
                dependency_value_bridge_target_prep_error(
                    error,
                    dependency_package,
                    &dependency_manifest.manifest_path,
                )
            })
        },
    )?;

    Ok(RenderedDependencyPublicValueDeclarations {
        declarations: declarations.join("\n\n"),
        required_functions_by_module_path,
        required_types_by_module_path,
    })
}
