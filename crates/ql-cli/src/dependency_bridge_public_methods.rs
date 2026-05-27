use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use ql_ast::Module;
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
use crate::dependency_bridge_modules::{
    dependency_interface_module_import_path, dependency_module_source_path,
};
use crate::dependency_bridge_names::render_imported_dependency_public_method_forwarder;
use crate::dependency_bridge_public_types::{
    collect_dependency_public_function_type_dependencies,
    dependency_public_struct_method_bridge_candidates, dependency_public_type_bridge_candidates,
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
                eprintln!(
                    "error: {command_label} failed to load referenced package interface `{}`: {error}",
                    normalize_path(&interface_path)
                );
                eprintln!(
                    "note: while preparing dependency public method bridges for `{}`",
                    normalize_path(manifest_path)
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
                        eprintln!(
                            "error: {command_label} failed to access dependency source `{}`: {error}",
                            normalize_path(dependency_source_path)
                        );
                        eprintln!(
                            "note: while preparing dependency public method bridges for `{}`",
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
                            "error: {command_label} failed to parse dependency source `{}` while preparing public method bridges",
                            normalize_path(dependency_source_path)
                        );
                        eprintln!("note: dependency package: `{dependency_package}`");
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

        collect_dependency_public_method_forwarders_from_modules(
            &dependency_package,
            &dependency_manifest.manifest_path,
            &artifact.modules,
            required_types_by_module_path,
            &mut discovered_required_types,
            &mut forwarders,
            |dependency_source_path| {
                fs::read_to_string(dependency_source_path).map_err(|error| {
                    PrepareProjectTargetBuildError {
                        failure_kind: PrepareProjectTargetBuildFailureKind::DependencySource {
                            dependency_manifest_path: dependency_manifest.manifest_path.clone(),
                            dependency_package: dependency_package.clone(),
                            source_path: dependency_source_path.to_path_buf(),
                            message: format!(
                                "failed to access dependency source `{}`: {error}",
                                normalize_path(dependency_source_path)
                            ),
                        },
                    }
                })
            },
            |dependency_source_path, dependency_source| {
                parse_source(dependency_source).map_err(|_| PrepareProjectTargetBuildError {
                    failure_kind: PrepareProjectTargetBuildFailureKind::DependencySource {
                        dependency_manifest_path: dependency_manifest.manifest_path.clone(),
                        dependency_package: dependency_package.clone(),
                        source_path: dependency_source_path.to_path_buf(),
                        message: format!(
                            "failed to parse dependency source `{}` while preparing public method bridges",
                            normalize_path(dependency_source_path)
                        ),
                    },
                })
            },
        )?;
    }

    Ok(RenderedDependencyPublicMethodForwarders {
        forwarders: forwarders.join("\n\n"),
        required_types_by_module_path: discovered_required_types,
    })
}

fn collect_dependency_public_method_forwarders_from_modules<E>(
    dependency_package: &str,
    dependency_manifest_path: &Path,
    modules: &[ql_project::InterfaceModule],
    required_types_by_module_path: &BTreeMap<Vec<String>, BTreeSet<String>>,
    discovered_required_types: &mut BTreeMap<Vec<String>, BTreeSet<String>>,
    forwarders: &mut Vec<String>,
    mut read_source: impl FnMut(&Path) -> Result<String, E>,
    mut parse_source_module: impl FnMut(&Path, &str) -> Result<Module, E>,
) -> Result<(), E> {
    for module in modules {
        let dependency_source_path =
            dependency_module_source_path(dependency_manifest_path, &module.source_path);
        let dependency_source = read_source(&dependency_source_path)?;
        let source_module = parse_source_module(&dependency_source_path, &dependency_source)?;
        collect_dependency_module_public_method_forwarders(
            dependency_package,
            &source_module,
            &dependency_source,
            required_types_by_module_path,
            discovered_required_types,
            forwarders,
        );
    }

    Ok(())
}

fn collect_dependency_module_public_method_forwarders(
    dependency_package: &str,
    module: &Module,
    contents: &str,
    required_types_by_module_path: &BTreeMap<Vec<String>, BTreeSet<String>>,
    discovered_required_types: &mut BTreeMap<Vec<String>, BTreeSet<String>>,
    forwarders: &mut Vec<String>,
) {
    let module_import_path = dependency_interface_module_import_path(dependency_package, module);
    let Some(initial_required_types) = required_types_by_module_path.get(&module_import_path)
    else {
        return;
    };
    let type_candidates = dependency_public_type_bridge_candidates(module);
    let mut pending_types = initial_required_types.clone();
    let mut processed_types = BTreeSet::new();
    let mut emitted_methods = BTreeSet::<(String, String)>::new();

    while let Some(struct_name) = pending_types.iter().next().cloned() {
        pending_types.remove(&struct_name);
        if !processed_types.insert(struct_name.clone())
            || !type_candidates.contains_key(&struct_name)
        {
            continue;
        }
        if type_candidates
            .get(&struct_name)
            .is_some_and(|candidate| candidate.decl.is_generic())
        {
            continue;
        }

        for (method_name, method) in
            dependency_public_struct_method_bridge_candidates(module, &struct_name)
        {
            let type_dependencies =
                collect_dependency_public_function_type_dependencies(method, &type_candidates);
            if !type_dependencies.is_empty() {
                let required_types = discovered_required_types
                    .entry(module_import_path.clone())
                    .or_default();
                for dependency in type_dependencies {
                    if required_types.insert(dependency.clone()) {
                        pending_types.insert(dependency);
                    }
                }
            }

            if !emitted_methods.insert((struct_name.clone(), method_name)) {
                continue;
            }

            if let Some(forwarder) = render_imported_dependency_public_method_forwarder(
                &module_import_path,
                &struct_name,
                method,
                contents,
            ) {
                forwarders.push(forwarder);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_method_forwarders_follow_required_type_dependencies() {
        let dependency_source = r#"
pub struct Box { value: Int }
pub struct Label { value: Int }

impl Box {
    pub fn label(self) -> Label {
        return Label { value: self.value }
    }
}

impl Label {
    pub fn read(self) -> Int {
        return self.value
    }
}
"#;
        let dependency_module = parse_source(dependency_source).unwrap();
        let module_import_path = dependency_interface_module_import_path("dep", &dependency_module);
        let mut required_types_by_module_path = BTreeMap::<Vec<String>, BTreeSet<String>>::new();
        required_types_by_module_path
            .entry(module_import_path.clone())
            .or_default()
            .insert("Box".to_owned());
        let mut discovered_required_types = BTreeMap::<Vec<String>, BTreeSet<String>>::new();
        let mut forwarders = Vec::new();

        collect_dependency_module_public_method_forwarders(
            "dep",
            &dependency_module,
            dependency_source,
            &required_types_by_module_path,
            &mut discovered_required_types,
            &mut forwarders,
        );

        assert_eq!(
            discovered_required_types.get(&module_import_path),
            Some(&BTreeSet::from(["Label".to_owned()]))
        );
        assert_eq!(forwarders.len(), 2);
        let rendered = forwarders.join("\n\n");
        assert!(rendered.contains("pub fn label(self) -> Label"));
        assert!(rendered.contains("pub fn read(self) -> Int"));
    }
}
