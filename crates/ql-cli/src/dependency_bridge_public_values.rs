use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use ql_ast::{ItemKind, Module, Visibility};
use ql_parser::parse_source;
use ql_project::{
    default_interface_path, load_interface_artifact, load_project_manifest,
    load_reference_manifests, package_name,
};

use crate::build_plan::{
    PrepareProjectTargetBuildError, PrepareProjectTargetBuildFailureKind,
    report_project_build_dependency_error, target_prep_dependency_interface_failure,
    target_prep_dependency_manifest_failure, target_prep_dependency_source_parse_failure,
    target_prep_dependency_source_read_failure,
};
use crate::dependency_bridge_imports::{
    ImportedDependencyExterns, collect_imported_dependency_externs,
    collect_top_level_definition_names, dependency_extern_is_imported,
};
use crate::dependency_bridge_modules::{
    dependency_interface_module_import_path, dependency_interface_module_import_paths,
    dependency_module_source_path,
};
use crate::dependency_bridge_names::{
    DependencyExternOwner, record_dependency_extern_declaration, span_text,
};
use crate::dependency_bridge_public_globals::{
    dependency_public_function_bridge_candidates, dependency_public_global_bridge_candidates,
    dependency_public_global_bridge_order, dependency_public_global_dependencies,
};
use crate::dependency_bridge_public_types::{
    collect_dependency_public_type_expr_dependencies, dependency_public_type_bridge_candidates,
};
use crate::dependency_bridge_reporting::{
    report_dependency_interface_load_failure, report_dependency_source_parse_failure,
    report_dependency_source_read_failure,
};
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
                    "dependency public value bridges",
                    &interface_path,
                    error,
                );
            }
            1
        })?;
        let module_import_paths =
            dependency_interface_module_import_paths(&dependency_package, &artifact.modules);
        let imported_externs =
            collect_imported_dependency_externs(&root_source_module, &module_import_paths);

        for module in &artifact.modules {
            let dependency_source_path =
                dependency_module_source_path(&dependency.manifest_path, &module.source_path);
            let dependency_source =
                fs::read_to_string(&dependency_source_path).map_err(|error| {
                    if report_failure {
                        report_dependency_source_read_failure(
                            command_label,
                            manifest_path,
                            "dependency public value bridges",
                            &dependency_source_path,
                            error,
                        );
                    }
                    1
                })?;
            let source_module = match parse_source(&dependency_source) {
                Ok(module) => module,
                Err(_) => {
                    if report_failure {
                        report_dependency_source_parse_failure(
                            command_label,
                            &dependency_package,
                            &dependency_source_path,
                            "public value bridges",
                        );
                    }
                    return Err(1);
                }
            };
            collect_dependency_module_public_value_declarations(
                &dependency_package,
                &dependency.manifest_path,
                &source_module,
                &dependency_source,
                Some(&imported_externs),
                &occupied_root_names,
                &mut required_functions_by_module_path,
                &mut required_types_by_module_path,
                &mut owners_by_symbol,
                &mut declarations,
            )
            .map_err(|error| {
                if report_failure {
                    match error {
                        DependencyPublicValueBridgeError::DependencyConflict { symbol, owner } => {
                            eprintln!(
                                "error: {command_label} found conflicting direct dependency public value imports for `{symbol}`"
                            );
                            eprintln!("note: first package: `{}`", owner.package_name);
                            eprintln!("note: conflicting package: `{dependency_package}`");
                            eprintln!(
                                "hint: keep direct dependency public value names unique until package-qualified dependency value lowering lands"
                            );
                        }
                        DependencyPublicValueBridgeError::LocalConflict { symbol } => {
                            eprintln!(
                                "error: {command_label} cannot synthesize direct dependency public value bridge for `{symbol}` because the root source already defines the same top-level name"
                            );
                            eprintln!("note: conflicting direct dependency package: `{dependency_package}`");
                            eprintln!(
                                "hint: rename the local top-level item or avoid importing a direct dependency public value with the same original symbol name"
                            );
                        }
                    }
                }
                1
            })?;
        }
    }

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
        let module_import_paths =
            dependency_interface_module_import_paths(&dependency_package, &artifact.modules);
        let imported_externs =
            collect_imported_dependency_externs(&root_source_module, &module_import_paths);

        for module in &artifact.modules {
            let dependency_source_path = dependency_module_source_path(
                &dependency_manifest.manifest_path,
                &module.source_path,
            );
            let dependency_source =
                fs::read_to_string(&dependency_source_path).map_err(|error| {
                    target_prep_dependency_source_read_failure(
                        &dependency_manifest.manifest_path,
                        &dependency_package,
                        &dependency_source_path,
                        error,
                    )
                })?;
            let source_module = parse_source(&dependency_source).map_err(|_| {
                target_prep_dependency_source_parse_failure(
                    &dependency_manifest.manifest_path,
                    &dependency_package,
                    &dependency_source_path,
                    "public value bridges",
                )
            })?;
            collect_dependency_module_public_value_declarations(
                &dependency_package,
                &dependency_manifest.manifest_path,
                &source_module,
                &dependency_source,
                Some(&imported_externs),
                &occupied_root_names,
                &mut required_functions_by_module_path,
                &mut required_types_by_module_path,
                &mut owners_by_symbol,
                &mut declarations,
            )
            .map_err(|error| match error {
                DependencyPublicValueBridgeError::DependencyConflict { symbol, owner } => {
                    PrepareProjectTargetBuildError {
                        failure_kind:
                            PrepareProjectTargetBuildFailureKind::DependencyValueConflict {
                                symbol,
                                first_package: owner.package_name,
                                first_manifest_path: owner.manifest_path,
                                conflicting_package: dependency_package.clone(),
                                conflicting_manifest_path: dependency_manifest
                                    .manifest_path
                                    .clone(),
                            },
                    }
                }
                DependencyPublicValueBridgeError::LocalConflict { symbol } => {
                    PrepareProjectTargetBuildError {
                        failure_kind:
                            PrepareProjectTargetBuildFailureKind::DependencyValueLocalConflict {
                                symbol,
                                dependency_package: dependency_package.clone(),
                                dependency_manifest_path: dependency_manifest.manifest_path.clone(),
                            },
                    }
                }
            })?;
        }
    }

    Ok(RenderedDependencyPublicValueDeclarations {
        declarations: declarations.join("\n\n"),
        required_functions_by_module_path,
        required_types_by_module_path,
    })
}

pub(crate) fn collect_dependency_module_public_value_declarations(
    dependency_package: &str,
    dependency_manifest_path: &Path,
    module: &Module,
    contents: &str,
    imported_externs: Option<&ImportedDependencyExterns>,
    occupied_root_names: &BTreeSet<String>,
    required_functions_by_module_path: &mut BTreeMap<Vec<String>, BTreeSet<String>>,
    required_types_by_module_path: &mut BTreeMap<Vec<String>, BTreeSet<String>>,
    owners_by_symbol: &mut BTreeMap<String, DependencyExternOwner>,
    declarations: &mut Vec<String>,
) -> Result<(), DependencyPublicValueBridgeError> {
    let module_import_path = dependency_interface_module_import_path(dependency_package, module);
    let global_candidates = dependency_public_global_bridge_candidates(module);
    let function_candidates = dependency_public_function_bridge_candidates(module);
    let type_candidates = dependency_public_type_bridge_candidates(module);
    let mut emitted = BTreeSet::new();

    for item in &module.items {
        let global = match &item.kind {
            ItemKind::Const(global) | ItemKind::Static(global)
                if global.visibility == Visibility::Public =>
            {
                global
            }
            _ => continue,
        };
        if imported_externs.is_some_and(|imports| {
            !dependency_extern_is_imported(imports, &module_import_path, &global.name)
        }) {
            continue;
        }

        let Some(ordered_symbols) = dependency_public_global_bridge_order(
            &global.name,
            &global_candidates,
            &function_candidates,
        ) else {
            continue;
        };

        for ordered_symbol in ordered_symbols {
            if emitted.contains(&ordered_symbol) {
                continue;
            }
            if occupied_root_names.contains(&ordered_symbol) {
                return Err(DependencyPublicValueBridgeError::LocalConflict {
                    symbol: ordered_symbol,
                });
            }
            let candidate = global_candidates
                .get(&ordered_symbol)
                .expect("ordered dependency public globals should resolve to candidates");
            let dependencies = dependency_public_global_dependencies(
                &ordered_symbol,
                &global_candidates,
                &function_candidates,
            )
            .expect("emitted dependency public globals should remain bridgeable");
            record_dependency_extern_declaration(
                dependency_package,
                dependency_manifest_path,
                &ordered_symbol,
                span_text(contents, candidate.item.span),
                owners_by_symbol,
                declarations,
            )
            .map_err(|(symbol, owner)| {
                DependencyPublicValueBridgeError::DependencyConflict { symbol, owner }
            })?;
            if !dependencies.functions.is_empty() {
                required_functions_by_module_path
                    .entry(module_import_path.clone())
                    .or_default()
                    .extend(dependencies.functions);
            }
            let mut type_dependencies = BTreeSet::new();
            collect_dependency_public_type_expr_dependencies(
                &candidate.global.ty,
                &type_candidates,
                &mut type_dependencies,
            );
            if !type_dependencies.is_empty() {
                required_types_by_module_path
                    .entry(module_import_path.clone())
                    .or_default()
                    .extend(type_dependencies);
            }
            emitted.insert(ordered_symbol);
        }
    }

    Ok(())
}

pub(crate) enum DependencyPublicValueBridgeError {
    DependencyConflict {
        symbol: String,
        owner: DependencyExternOwner,
    },
    LocalConflict {
        symbol: String,
    },
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn module_value_declarations_report_local_conflicts() {
        let dependency_source = "pub const VALUE: Int = 1\n";
        let dependency_module = parse_source(dependency_source).unwrap();
        let root_source = "const VALUE: Int = 2\n";
        let root_module = parse_source(root_source).unwrap();
        let occupied_root_names = collect_top_level_definition_names(&root_module);
        let mut owners_by_symbol = BTreeMap::<String, DependencyExternOwner>::new();
        let mut required_functions_by_module_path =
            BTreeMap::<Vec<String>, BTreeSet<String>>::new();
        let mut required_types_by_module_path = BTreeMap::<Vec<String>, BTreeSet<String>>::new();
        let mut declarations = Vec::new();

        let error = collect_dependency_module_public_value_declarations(
            "dep",
            &PathBuf::from("dep/qlang.toml"),
            &dependency_module,
            dependency_source,
            None,
            &occupied_root_names,
            &mut required_functions_by_module_path,
            &mut required_types_by_module_path,
            &mut owners_by_symbol,
            &mut declarations,
        )
        .unwrap_err();

        assert!(matches!(
            error,
            DependencyPublicValueBridgeError::LocalConflict { symbol } if symbol == "VALUE"
        ));
        assert!(declarations.is_empty());
        assert!(required_functions_by_module_path.is_empty());
        assert!(required_types_by_module_path.is_empty());
    }

    #[test]
    fn module_value_declarations_report_dependency_conflicts() {
        let dependency_source = "pub const VALUE: Int = 1\n";
        let dependency_module = parse_source(dependency_source).unwrap();
        let occupied_root_names = BTreeSet::new();
        let mut owners_by_symbol = BTreeMap::<String, DependencyExternOwner>::new();
        let mut required_functions_by_module_path =
            BTreeMap::<Vec<String>, BTreeSet<String>>::new();
        let mut required_types_by_module_path = BTreeMap::<Vec<String>, BTreeSet<String>>::new();
        let mut declarations = Vec::new();

        let first_result = collect_dependency_module_public_value_declarations(
            "first",
            &PathBuf::from("first/qlang.toml"),
            &dependency_module,
            dependency_source,
            None,
            &occupied_root_names,
            &mut required_functions_by_module_path,
            &mut required_types_by_module_path,
            &mut owners_by_symbol,
            &mut declarations,
        );
        assert!(first_result.is_ok());
        let error = collect_dependency_module_public_value_declarations(
            "second",
            &PathBuf::from("second/qlang.toml"),
            &dependency_module,
            dependency_source,
            None,
            &occupied_root_names,
            &mut required_functions_by_module_path,
            &mut required_types_by_module_path,
            &mut owners_by_symbol,
            &mut declarations,
        )
        .unwrap_err();

        assert!(matches!(
            error,
            DependencyPublicValueBridgeError::DependencyConflict { symbol, owner }
                if symbol == "VALUE"
                    && owner.package_name == "first"
                    && owner.manifest_path == PathBuf::from("first/qlang.toml")
        ));
        assert_eq!(declarations.len(), 1);
    }
}
