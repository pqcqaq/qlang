use std::collections::BTreeMap;
use std::path::Path;

use ql_ast::{ItemKind, Module, Visibility};
use ql_parser::parse_source;
use ql_project::{
    default_interface_path, load_interface_artifact, load_project_manifest,
    load_reference_manifests, package_name,
};

use crate::build_plan::{
    PrepareProjectTargetBuildError, PrepareProjectTargetBuildFailureKind,
    report_project_build_dependency_error, target_prep_dependency_manifest_failure,
};
use crate::cli_utils::normalize_path;
use crate::dependency_bridge_imports::{
    ImportedDependencyExterns, collect_imported_dependency_externs, dependency_extern_is_imported,
};
use crate::dependency_bridge_modules::dependency_interface_module_import_paths;
use crate::dependency_bridge_names::{
    DependencyExternOwner, record_dependency_extern_declaration, span_text,
};
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
                eprintln!(
                    "error: {command_label} failed to load referenced package interface `{}`: {error}",
                    normalize_path(&interface_path)
                );
                eprintln!(
                    "note: while preparing dependency extern declarations for `{}`",
                    normalize_path(manifest_path)
                );
            }
            1
        })?;
        let module_import_paths =
            dependency_interface_module_import_paths(&dependency_package, &artifact.modules);
        let imported_externs =
            collect_imported_dependency_externs(&root_source_module, &module_import_paths);

        for module in &artifact.modules {
            collect_dependency_module_extern_declarations(
                &dependency_package,
                &dependency.manifest_path,
                &module.syntax,
                &module.contents,
                Some(&imported_externs),
                &mut owners_by_symbol,
                &mut declarations,
            )
            .map_err(|(symbol, owner)| {
                if report_failure {
                    eprintln!(
                        "error: {command_label} found conflicting direct dependency extern imports for `{symbol}`"
                    );
                    eprintln!("note: first package: `{}`", owner.package_name);
                    eprintln!("note: conflicting package: `{dependency_package}`");
                    eprintln!(
                        "hint: keep direct dependency `extern \"c\"` names unique until package-qualified extern resolution lands"
                    );
                }
                1
            })?;
        }
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
            PrepareProjectTargetBuildError {
                failure_kind: PrepareProjectTargetBuildFailureKind::DependencyInterface {
                    dependency_manifest_path: dependency_manifest.manifest_path.clone(),
                    dependency_package: dependency_package.clone(),
                    interface_path: interface_path.clone(),
                    message: format!(
                        "failed to load referenced package interface `{}`: {error}",
                        normalize_path(&interface_path)
                    ),
                },
            }
        })?;
        let module_import_paths =
            dependency_interface_module_import_paths(&dependency_package, &artifact.modules);
        let imported_externs =
            collect_imported_dependency_externs(&root_source_module, &module_import_paths);

        for module in &artifact.modules {
            collect_dependency_module_extern_declarations(
                &dependency_package,
                &dependency_manifest.manifest_path,
                &module.syntax,
                &module.contents,
                Some(&imported_externs),
                &mut owners_by_symbol,
                &mut declarations,
            )
            .map_err(|(symbol, owner)| PrepareProjectTargetBuildError {
                failure_kind: PrepareProjectTargetBuildFailureKind::DependencyExternConflict {
                    symbol,
                    first_package: owner.package_name,
                    first_manifest_path: owner.manifest_path,
                    conflicting_package: dependency_package.clone(),
                    conflicting_manifest_path: dependency_manifest.manifest_path.clone(),
                },
            })?;
        }
    }

    Ok(declarations.join("\n\n"))
}

fn collect_dependency_module_extern_declarations(
    dependency_package: &str,
    dependency_manifest_path: &Path,
    module: &Module,
    contents: &str,
    imported_externs: Option<&ImportedDependencyExterns>,
    owners_by_symbol: &mut BTreeMap<String, DependencyExternOwner>,
    declarations: &mut Vec<String>,
) -> Result<(), (String, DependencyExternOwner)> {
    let module_import_path =
        crate::dependency_bridge_modules::dependency_interface_module_import_path(
            dependency_package,
            module,
        );

    for item in &module.items {
        match &item.kind {
            ItemKind::Function(function)
                if function.visibility == Visibility::Public
                    && function.abi.as_deref() == Some("c")
                    && imported_externs.is_none_or(|imports| {
                        dependency_extern_is_imported(imports, &module_import_path, &function.name)
                    }) =>
            {
                record_dependency_extern_declaration(
                    dependency_package,
                    dependency_manifest_path,
                    &function.name,
                    span_text(contents, item.span),
                    owners_by_symbol,
                    declarations,
                )?;
            }
            ItemKind::ExternBlock(extern_block)
                if extern_block.visibility == Visibility::Public && extern_block.abi == "c" =>
            {
                for function in &extern_block.functions {
                    if imported_externs.is_some_and(|imports| {
                        !dependency_extern_is_imported(imports, &module_import_path, &function.name)
                    }) {
                        continue;
                    }
                    let mut declaration = String::from("extern \"c\" pub ");
                    declaration.push_str(span_text(contents, function.span).trim());
                    record_dependency_extern_declaration(
                        dependency_package,
                        dependency_manifest_path,
                        &function.name,
                        declaration,
                        owners_by_symbol,
                        declarations,
                    )?;
                }
            }
            _ => {}
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
    fn module_extern_declarations_report_conflicting_symbols() {
        let source =
            "extern \"c\" pub fn q_add(left: Int, right: Int) -> Int { return left + right }\n";
        let module = parse_source(source).unwrap();
        let mut owners_by_symbol = BTreeMap::<String, DependencyExternOwner>::new();
        let mut declarations = Vec::new();

        collect_dependency_module_extern_declarations(
            "first",
            &PathBuf::from("first/qlang.toml"),
            &module,
            source,
            None,
            &mut owners_by_symbol,
            &mut declarations,
        )
        .unwrap();
        let error = collect_dependency_module_extern_declarations(
            "second",
            &PathBuf::from("second/qlang.toml"),
            &module,
            source,
            None,
            &mut owners_by_symbol,
            &mut declarations,
        )
        .unwrap_err();

        assert_eq!(error.0, "q_add");
        assert_eq!(error.1.package_name, "first");
        assert_eq!(declarations.len(), 1);
    }
}
