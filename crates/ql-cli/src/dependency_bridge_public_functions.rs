use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use ql_ast::{ItemKind, Module};
use ql_parser::parse_source;
use ql_project::{
    default_interface_path, load_interface_artifact, load_project_manifest,
    load_reference_manifests, package_name,
};

use crate::build_plan::{
    report_project_build_dependency_error, target_prep_dependency_interface_failure,
    target_prep_dependency_manifest_failure, target_prep_dependency_source_parse_failure,
    target_prep_dependency_source_read_failure, PrepareProjectTargetBuildError,
    PrepareProjectTargetBuildFailureKind,
};
use crate::cli_utils::normalize_path;
use crate::dependency_bridge_imports::{
    collect_imported_dependency_externs, collect_top_level_definition_names,
    dependency_extern_is_imported, ImportedDependencyExterns,
};
use crate::dependency_bridge_modules::{
    dependency_generic_specialization_module_refs, dependency_generic_specialization_modules,
    dependency_generic_specialization_modules_quiet, dependency_interface_module_import_path,
    dependency_interface_module_import_paths, dependency_module_source_path,
};
use crate::dependency_bridge_names::{
    record_dependency_extern_declaration, render_imported_dependency_public_function_forwarder,
    supports_dependency_public_function_import_bridge, DependencyExternOwner,
};
use crate::dependency_bridge_public_types::{
    collect_dependency_public_function_type_dependencies, dependency_public_type_bridge_candidates,
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
                eprintln!(
                    "error: {command_label} failed to load referenced package interface `{}`: {error}",
                    normalize_path(&interface_path)
                );
                eprintln!(
                    "note: while preparing dependency public function wrappers for `{}`",
                    normalize_path(manifest_path)
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
                        eprintln!(
                            "error: {command_label} failed to access dependency source `{}`: {error}",
                            normalize_path(dependency_source_path)
                        );
                        eprintln!(
                            "note: while preparing dependency public function wrappers for `{}`",
                            normalize_path(manifest_path)
                        );
                    }
                    1
                })
            },
            |dependency_source_path, dependency_source| {
                parse_source(dependency_source).map_err(|_| {
                    if report_failure {
                        eprintln!(
                            "error: {command_label} failed to parse dependency source `{}` while preparing public function wrappers",
                            normalize_path(dependency_source_path)
                        );
                        eprintln!("note: dependency package: `{dependency_package}`");
                    }
                    1
                })
            },
            |error| {
                if report_failure {
                    match error {
                        DependencyPublicFunctionForwarderError::DependencyConflict {
                            symbol,
                            owner,
                        } => {
                            eprintln!(
                                "error: {command_label} found conflicting direct dependency public function imports for `{symbol}`"
                            );
                            eprintln!("note: first package: `{}`", owner.package_name);
                            eprintln!("note: conflicting package: `{dependency_package}`");
                            eprintln!(
                                "hint: keep direct dependency public function names unique until package-qualified dependency call lowering lands"
                            );
                        }
                        DependencyPublicFunctionForwarderError::LocalConflict { symbol } => {
                            eprintln!(
                                "error: {command_label} cannot synthesize direct dependency public function bridge for `{symbol}` because the root source already defines the same top-level name"
                            );
                            eprintln!(
                                "note: conflicting direct dependency package: `{dependency_package}`"
                            );
                            eprintln!(
                                "hint: rename the local top-level item or avoid importing a direct dependency public function with the same original symbol name"
                            );
                        }
                        DependencyPublicFunctionForwarderError::UnsupportedGeneric { symbol } => {
                            eprintln!(
                                "error: {command_label} cannot synthesize direct dependency public function bridge for generic function `{symbol}` yet"
                            );
                            eprintln!("note: direct dependency package: `{dependency_package}`");
                            eprintln!(
                                "hint: generic function monomorphization is not implemented yet; use a non-generic wrapper with concrete parameter and return types"
                            );
                        }
                    }
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

fn collect_dependency_public_function_forwarders_from_modules<E>(
    dependency_package: &str,
    dependency_manifest_path: &Path,
    modules: &[ql_project::InterfaceModule],
    root_source_module: &Module,
    required_functions_by_module_path: &BTreeMap<Vec<String>, BTreeSet<String>>,
    occupied_root_names: &BTreeSet<String>,
    specialization_modules: &[dependency_generic_bridge::SpecializationModule<'_>],
    required_types_by_module_path: &mut BTreeMap<Vec<String>, BTreeSet<String>>,
    owners_by_symbol: &mut BTreeMap<String, DependencyExternOwner>,
    forwarders: &mut Vec<String>,
    source_rewrites: &mut Vec<dependency_generic_bridge::SourceRewrite>,
    rendered_specializations: &mut BTreeSet<String>,
    mut read_source: impl FnMut(&Path) -> Result<String, E>,
    mut parse_source_module: impl FnMut(&Path, &str) -> Result<Module, E>,
    mut map_bridge_error: impl FnMut(DependencyPublicFunctionForwarderError) -> E,
) -> Result<(), E> {
    let module_import_paths = dependency_interface_module_import_paths(dependency_package, modules);
    let imported_externs =
        collect_imported_dependency_externs(root_source_module, &module_import_paths);

    for module in modules {
        let dependency_source_path =
            dependency_module_source_path(dependency_manifest_path, &module.source_path);
        let dependency_source = read_source(&dependency_source_path)?;
        let source_module = parse_source_module(&dependency_source_path, &dependency_source)?;
        let module_import_path =
            dependency_interface_module_import_path(dependency_package, &source_module);
        collect_dependency_module_public_function_forwarders(
            dependency_package,
            dependency_manifest_path,
            &source_module,
            &dependency_source,
            root_source_module,
            Some(&imported_externs),
            required_functions_by_module_path.get(&module_import_path),
            occupied_root_names,
            specialization_modules,
            required_types_by_module_path,
            owners_by_symbol,
            forwarders,
            source_rewrites,
            rendered_specializations,
        )
        .map_err(&mut map_bridge_error)?;
    }

    Ok(())
}

pub(crate) fn collect_dependency_module_public_function_forwarders(
    dependency_package: &str,
    dependency_manifest_path: &Path,
    module: &Module,
    contents: &str,
    root_module: &Module,
    imported_externs: Option<&ImportedDependencyExterns>,
    required_function_names: Option<&BTreeSet<String>>,
    occupied_root_names: &BTreeSet<String>,
    specialization_modules: &[dependency_generic_bridge::SpecializationModule<'_>],
    required_types_by_module_path: &mut BTreeMap<Vec<String>, BTreeSet<String>>,
    owners_by_symbol: &mut BTreeMap<String, DependencyExternOwner>,
    forwarders: &mut Vec<String>,
    source_rewrites: &mut Vec<dependency_generic_bridge::SourceRewrite>,
    rendered_specializations: &mut BTreeSet<String>,
) -> Result<(), DependencyPublicFunctionForwarderError> {
    let module_import_path = dependency_interface_module_import_path(dependency_package, module);
    let type_candidates = dependency_public_type_bridge_candidates(module);

    for item in &module.items {
        let ItemKind::Function(function) = &item.kind else {
            continue;
        };
        let imported = imported_externs.is_none_or(|imports| {
            dependency_extern_is_imported(imports, &module_import_path, &function.name)
        });
        let required_by_value = required_function_names.is_some_and(|required_function_names| {
            required_function_names.contains(&function.name)
        });
        if !imported && !required_by_value {
            continue;
        }
        if dependency_generic_bridge::supports_public_function_specialization(function) {
            let rendered =
                match dependency_generic_bridge::render_public_function_specialization_status_with_context(
                    &module_import_path,
                    function,
                    contents,
                    root_module,
                    module,
                    specialization_modules,
                    rendered_specializations,
                ) {
                    dependency_generic_bridge::PublicFunctionSpecializationRender::Rendered(
                        rendered,
                    ) => rendered,
                    dependency_generic_bridge::PublicFunctionSpecializationRender::NotCalled
                        if !required_by_value =>
                    {
                        continue;
                    }
                    dependency_generic_bridge::PublicFunctionSpecializationRender::NotCalled
                    | dependency_generic_bridge::PublicFunctionSpecializationRender::Unsupported => {
                        return Err(DependencyPublicFunctionForwarderError::UnsupportedGeneric {
                            symbol: function.name.clone(),
                        });
                    }
                };
            record_dependency_extern_declaration(
                dependency_package,
                dependency_manifest_path,
                &function.name,
                rendered.declarations,
                owners_by_symbol,
                forwarders,
            )
            .map_err(|(symbol, owner)| {
                DependencyPublicFunctionForwarderError::DependencyConflict { symbol, owner }
            })?;
            let type_dependencies =
                collect_dependency_public_function_type_dependencies(function, &type_candidates);
            if !type_dependencies.is_empty() {
                required_types_by_module_path
                    .entry(module_import_path.clone())
                    .or_default()
                    .extend(type_dependencies);
            }
            source_rewrites.extend(rendered.call_rewrites);
            continue;
        }
        if !supports_dependency_public_function_import_bridge(function) {
            continue;
        }
        let type_dependencies =
            collect_dependency_public_function_type_dependencies(function, &type_candidates);
        if occupied_root_names.contains(&function.name) {
            return Err(DependencyPublicFunctionForwarderError::LocalConflict {
                symbol: function.name.clone(),
            });
        }
        let Some(forwarder) = render_imported_dependency_public_function_forwarder(
            &module_import_path,
            function,
            contents,
        ) else {
            continue;
        };
        record_dependency_extern_declaration(
            dependency_package,
            dependency_manifest_path,
            &function.name,
            forwarder,
            owners_by_symbol,
            forwarders,
        )
        .map_err(|(symbol, owner)| {
            DependencyPublicFunctionForwarderError::DependencyConflict { symbol, owner }
        })?;
        if !type_dependencies.is_empty() {
            required_types_by_module_path
                .entry(module_import_path.clone())
                .or_default()
                .extend(type_dependencies);
        }
    }

    Ok(())
}

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

fn dependency_function_forwarder_target_prep_error(
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
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn module_function_forwarders_report_local_conflicts() {
        let dependency_source = "pub fn add(value: Int) -> Int { return value + 1 }\n";
        let dependency_module = parse_source(dependency_source).unwrap();
        let root_source = "fn add(value: Int) -> Int { return value }\n";
        let root_module = parse_source(root_source).unwrap();
        let occupied_root_names = collect_top_level_definition_names(&root_module);
        let mut owners_by_symbol = BTreeMap::<String, DependencyExternOwner>::new();
        let mut required_types_by_module_path = BTreeMap::<Vec<String>, BTreeSet<String>>::new();
        let mut forwarders = Vec::new();
        let mut source_rewrites = Vec::new();
        let mut rendered_specializations = BTreeSet::new();

        let error = collect_dependency_module_public_function_forwarders(
            "dep",
            &PathBuf::from("dep/qlang.toml"),
            &dependency_module,
            dependency_source,
            &root_module,
            None,
            None,
            &occupied_root_names,
            &[],
            &mut required_types_by_module_path,
            &mut owners_by_symbol,
            &mut forwarders,
            &mut source_rewrites,
            &mut rendered_specializations,
        )
        .unwrap_err();

        assert!(matches!(
            error,
            DependencyPublicFunctionForwarderError::LocalConflict { symbol } if symbol == "add"
        ));
        assert!(forwarders.is_empty());
        assert!(source_rewrites.is_empty());
        assert!(required_types_by_module_path.is_empty());
    }

    #[test]
    fn module_function_forwarders_report_dependency_conflicts() {
        let dependency_source = "pub fn add(value: Int) -> Int { return value + 1 }\n";
        let dependency_module = parse_source(dependency_source).unwrap();
        let root_source = "";
        let root_module = parse_source(root_source).unwrap();
        let occupied_root_names = BTreeSet::new();
        let mut owners_by_symbol = BTreeMap::<String, DependencyExternOwner>::new();
        let mut required_types_by_module_path = BTreeMap::<Vec<String>, BTreeSet<String>>::new();
        let mut forwarders = Vec::new();
        let mut source_rewrites = Vec::new();
        let mut rendered_specializations = BTreeSet::new();

        let first_result = collect_dependency_module_public_function_forwarders(
            "first",
            &PathBuf::from("first/qlang.toml"),
            &dependency_module,
            dependency_source,
            &root_module,
            None,
            None,
            &occupied_root_names,
            &[],
            &mut required_types_by_module_path,
            &mut owners_by_symbol,
            &mut forwarders,
            &mut source_rewrites,
            &mut rendered_specializations,
        );
        assert!(first_result.is_ok());
        let error = collect_dependency_module_public_function_forwarders(
            "second",
            &PathBuf::from("second/qlang.toml"),
            &dependency_module,
            dependency_source,
            &root_module,
            None,
            None,
            &occupied_root_names,
            &[],
            &mut required_types_by_module_path,
            &mut owners_by_symbol,
            &mut forwarders,
            &mut source_rewrites,
            &mut rendered_specializations,
        )
        .unwrap_err();

        assert!(matches!(
            error,
            DependencyPublicFunctionForwarderError::DependencyConflict { symbol, owner }
                if symbol == "add"
                    && owner.package_name == "first"
                    && owner.manifest_path == PathBuf::from("first/qlang.toml")
        ));
        assert_eq!(forwarders.len(), 1);
    }
}
