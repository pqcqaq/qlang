use std::path::Path;

use ql_ast::Module;
use ql_project::{
    InterfaceError, InterfaceModule, ProjectError, ProjectManifest, default_interface_path,
    load_interface_artifact, package_name,
};

use crate::dependency_bridge_imports::{
    ImportedDependencyExterns, collect_imported_dependency_externs,
};
use crate::dependency_bridge_modules::{
    dependency_interface_module_import_path, dependency_interface_module_import_paths,
    dependency_module_source_path,
};

pub(crate) fn collect_dependency_interface_module_bridge_items<E>(
    dependency_package: &str,
    interface_modules: &[InterfaceModule],
    root_source_module: &Module,
    mut collect_interface_module: impl FnMut(
        &InterfaceModule,
        &ImportedDependencyExterns,
        &Vec<String>,
    ) -> Result<(), E>,
) -> Result<(), E> {
    let module_import_paths =
        dependency_interface_module_import_paths(dependency_package, interface_modules);
    let imported_externs =
        collect_imported_dependency_externs(root_source_module, &module_import_paths);

    for module in interface_modules {
        let module_import_path =
            dependency_interface_module_import_path(dependency_package, &module.syntax);
        collect_interface_module(module, &imported_externs, &module_import_path)?;
    }

    Ok(())
}

pub(crate) fn collect_dependency_source_modules<E>(
    dependency_manifest_path: &Path,
    interface_modules: &[InterfaceModule],
    mut read_source: impl FnMut(&Path) -> Result<String, E>,
    mut parse_source_module: impl FnMut(&Path, &str) -> Result<Module, E>,
    mut collect_source_module: impl FnMut(&Module, &str) -> Result<(), E>,
) -> Result<(), E> {
    for module in interface_modules {
        let dependency_source_path =
            dependency_module_source_path(dependency_manifest_path, &module.source_path);
        let dependency_source = read_source(&dependency_source_path)?;
        let source_module = parse_source_module(&dependency_source_path, &dependency_source)?;
        collect_source_module(&source_module, &dependency_source)?;
    }

    Ok(())
}

pub(crate) fn collect_dependency_source_module_bridge_items<E>(
    dependency_package: &str,
    dependency_manifest_path: &Path,
    interface_modules: &[InterfaceModule],
    root_source_module: &Module,
    mut read_source: impl FnMut(&Path) -> Result<String, E>,
    mut parse_source_module: impl FnMut(&Path, &str) -> Result<Module, E>,
    mut collect_source_module: impl FnMut(
        &Module,
        &str,
        &ImportedDependencyExterns,
        &Vec<String>,
    ) -> Result<(), E>,
) -> Result<(), E> {
    collect_dependency_interface_module_bridge_items(
        dependency_package,
        interface_modules,
        root_source_module,
        |interface_module, imported_externs, _interface_module_import_path| {
            let source_path = dependency_module_source_path(
                dependency_manifest_path,
                &interface_module.source_path,
            );
            let dependency_source = read_source(&source_path)?;
            let source_module = parse_source_module(&source_path, &dependency_source)?;
            let module_import_path =
                dependency_interface_module_import_path(dependency_package, &source_module);
            collect_source_module(
                &source_module,
                &dependency_source,
                imported_externs,
                &module_import_path,
            )
        },
    )
}

pub(crate) fn collect_direct_dependency_source_module_bridge_items<E>(
    direct_dependencies: &[ProjectManifest],
    root_source_module: &Module,
    mut map_package_name_error: impl FnMut(&ProjectManifest, &ProjectError) -> E,
    mut map_interface_path_error: impl FnMut(&ProjectManifest, &ProjectError) -> E,
    mut map_interface_load_error: impl FnMut(&ProjectManifest, &str, &Path, InterfaceError) -> E,
    mut read_source: impl FnMut(&ProjectManifest, &str, &Path) -> Result<String, E>,
    mut parse_source_module: impl FnMut(&ProjectManifest, &str, &Path, &str) -> Result<Module, E>,
    mut collect_source_module: impl FnMut(
        &ProjectManifest,
        &str,
        &Module,
        &str,
        &ImportedDependencyExterns,
        &Vec<String>,
    ) -> Result<(), E>,
) -> Result<(), E> {
    for dependency in direct_dependencies {
        let dependency_package = package_name(dependency)
            .map(str::to_owned)
            .map_err(|error| map_package_name_error(dependency, &error))?;
        let interface_path = default_interface_path(dependency)
            .map_err(|error| map_interface_path_error(dependency, &error))?;
        let artifact = load_interface_artifact(&interface_path).map_err(|error| {
            map_interface_load_error(dependency, &dependency_package, &interface_path, error)
        })?;
        collect_dependency_source_module_bridge_items(
            &dependency_package,
            &dependency.manifest_path,
            &artifact.modules,
            root_source_module,
            |dependency_source_path| {
                read_source(dependency, &dependency_package, dependency_source_path)
            },
            |dependency_source_path, dependency_source| {
                parse_source_module(
                    dependency,
                    &dependency_package,
                    dependency_source_path,
                    dependency_source,
                )
            },
            |source_module, dependency_source, imported_externs, module_import_path| {
                collect_source_module(
                    dependency,
                    &dependency_package,
                    source_module,
                    dependency_source,
                    imported_externs,
                    module_import_path,
                )
            },
        )?;
    }

    Ok(())
}

#[cfg(test)]
#[path = "dependency_bridge_source_modules_tests.rs"]
mod tests;
