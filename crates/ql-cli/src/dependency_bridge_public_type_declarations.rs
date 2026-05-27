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
    PrepareProjectTargetBuildError, report_project_build_dependency_error,
    target_prep_dependency_interface_failure, target_prep_dependency_manifest_failure,
    target_prep_dependency_source_parse_failure, target_prep_dependency_source_read_failure,
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
use crate::dependency_bridge_public_type_errors::{
    DependencyPublicTypeBridgeError, dependency_type_bridge_target_prep_error,
    report_direct_dependency_type_bridge_error,
};
use crate::dependency_bridge_public_types::{
    dependency_public_type_bridge_candidates, dependency_public_type_bridge_order,
};
use crate::dependency_bridge_reporting::{
    report_dependency_interface_load_failure, report_dependency_source_parse_failure,
    report_dependency_source_read_failure,
};
use crate::project_manifest_paths::reference_manifest_path;

pub(crate) fn render_direct_dependency_public_type_declarations(
    command_label: &str,
    manifest_path: &Path,
    source: &str,
    required_types_by_module_path: &BTreeMap<Vec<String>, BTreeSet<String>>,
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
    let occupied_root_names = collect_top_level_definition_names(&root_source_module);

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
                    "dependency public type bridges",
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
                            "dependency public type bridges",
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
                            "public type bridges",
                        );
                    }
                    return Err(1);
                }
            };
            let module_import_path =
                dependency_interface_module_import_path(&dependency_package, &source_module);
            collect_dependency_module_public_type_declarations(
                &dependency_package,
                &dependency.manifest_path,
                &source_module,
                &dependency_source,
                Some(&imported_externs),
                required_types_by_module_path.get(&module_import_path),
                &occupied_root_names,
                &mut owners_by_symbol,
                &mut declarations,
            )
            .map_err(|error| {
                if report_failure {
                    report_direct_dependency_type_bridge_error(
                        command_label,
                        &dependency_package,
                        error,
                    );
                }
                1
            })?;
        }
    }

    Ok(declarations.join("\n\n"))
}

pub(crate) fn render_direct_dependency_public_type_declarations_quiet(
    manifest_path: &Path,
    source: &str,
    required_types_by_module_path: &BTreeMap<Vec<String>, BTreeSet<String>>,
) -> Result<String, PrepareProjectTargetBuildError> {
    let manifest = load_project_manifest(manifest_path)
        .map_err(|error| target_prep_dependency_manifest_failure(None, &error))?;
    let manifest_dir = manifest.manifest_path.parent().unwrap_or(Path::new("."));
    let root_source_module = match parse_source(source) {
        Ok(module) => module,
        Err(_) => return Ok(String::new()),
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
                    "public type bridges",
                )
            })?;
            let module_import_path =
                dependency_interface_module_import_path(&dependency_package, &source_module);
            collect_dependency_module_public_type_declarations(
                &dependency_package,
                &dependency_manifest.manifest_path,
                &source_module,
                &dependency_source,
                Some(&imported_externs),
                required_types_by_module_path.get(&module_import_path),
                &occupied_root_names,
                &mut owners_by_symbol,
                &mut declarations,
            )
            .map_err(|error| {
                dependency_type_bridge_target_prep_error(
                    error,
                    &dependency_package,
                    &dependency_manifest.manifest_path,
                )
            })?;
        }
    }

    Ok(declarations.join("\n\n"))
}

pub(crate) fn collect_dependency_module_public_type_declarations(
    dependency_package: &str,
    dependency_manifest_path: &Path,
    module: &Module,
    contents: &str,
    imported_externs: Option<&ImportedDependencyExterns>,
    required_type_names: Option<&BTreeSet<String>>,
    occupied_root_names: &BTreeSet<String>,
    owners_by_symbol: &mut BTreeMap<String, DependencyExternOwner>,
    declarations: &mut Vec<String>,
) -> Result<(), DependencyPublicTypeBridgeError> {
    let module_import_path = dependency_interface_module_import_path(dependency_package, module);
    let type_candidates = dependency_public_type_bridge_candidates(module);
    let mut emitted = BTreeSet::new();

    for item in &module.items {
        let type_name = match &item.kind {
            ItemKind::Struct(struct_decl) if struct_decl.visibility == Visibility::Public => {
                struct_decl.name.as_str()
            }
            ItemKind::Enum(enum_decl) if enum_decl.visibility == Visibility::Public => {
                enum_decl.name.as_str()
            }
            ItemKind::TypeAlias(alias)
                if alias.visibility == Visibility::Public
                    && !alias.is_opaque
                    && alias.generics.is_empty() =>
            {
                alias.name.as_str()
            }
            _ => continue,
        };
        let imported = imported_externs.is_none_or(|imports| {
            dependency_extern_is_imported(imports, &module_import_path, type_name)
        });
        let required_by_bridge = required_type_names
            .is_some_and(|required_type_names| required_type_names.contains(type_name));
        if !imported && !required_by_bridge {
            continue;
        }

        let Some(ordered_symbols) =
            dependency_public_type_bridge_order(type_name, &type_candidates)
        else {
            continue;
        };

        for ordered_symbol in ordered_symbols {
            if emitted.contains(&ordered_symbol) {
                continue;
            }
            if occupied_root_names.contains(&ordered_symbol) {
                return Err(DependencyPublicTypeBridgeError::LocalConflict {
                    symbol: ordered_symbol,
                });
            }
            let candidate = type_candidates
                .get(&ordered_symbol)
                .expect("ordered dependency public types should resolve to candidates");
            record_dependency_extern_declaration(
                dependency_package,
                dependency_manifest_path,
                &ordered_symbol,
                span_text(contents, candidate.item.span),
                owners_by_symbol,
                declarations,
            )
            .map_err(|(symbol, owner)| {
                DependencyPublicTypeBridgeError::DependencyConflict { symbol, owner }
            })?;
            emitted.insert(ordered_symbol);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use ql_parser::parse_source;

    use super::*;

    #[test]
    fn module_type_declarations_report_local_conflicts() {
        let dependency_source = "pub struct Box { value: Int }\n";
        let dependency_module = parse_source(dependency_source).unwrap();
        let root_source = "struct Box { value: Int }\n";
        let root_module = parse_source(root_source).unwrap();
        let occupied_root_names = collect_top_level_definition_names(&root_module);
        let mut owners_by_symbol = BTreeMap::<String, DependencyExternOwner>::new();
        let mut declarations = Vec::new();

        let error = collect_dependency_module_public_type_declarations(
            "dep",
            &PathBuf::from("dep/qlang.toml"),
            &dependency_module,
            dependency_source,
            None,
            None,
            &occupied_root_names,
            &mut owners_by_symbol,
            &mut declarations,
        )
        .unwrap_err();

        assert!(matches!(
            error,
            DependencyPublicTypeBridgeError::LocalConflict { symbol } if symbol == "Box"
        ));
        assert!(declarations.is_empty());
    }
}
