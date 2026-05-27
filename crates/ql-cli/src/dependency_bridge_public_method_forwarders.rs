use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use ql_ast::Module;

use crate::dependency_bridge_modules::dependency_interface_module_import_path;
use crate::dependency_bridge_names::render_imported_dependency_public_method_forwarder;
use crate::dependency_bridge_public_types::{
    collect_dependency_public_function_type_dependencies,
    dependency_public_struct_method_bridge_candidates, dependency_public_type_bridge_candidates,
};
use crate::dependency_bridge_source_modules::collect_dependency_source_modules;

pub(crate) fn collect_dependency_public_method_forwarders_from_modules<E>(
    dependency_package: &str,
    dependency_manifest_path: &Path,
    modules: &[ql_project::InterfaceModule],
    required_types_by_module_path: &BTreeMap<Vec<String>, BTreeSet<String>>,
    discovered_required_types: &mut BTreeMap<Vec<String>, BTreeSet<String>>,
    forwarders: &mut Vec<String>,
    read_source: impl FnMut(&Path) -> Result<String, E>,
    parse_source_module: impl FnMut(&Path, &str) -> Result<Module, E>,
) -> Result<(), E> {
    collect_dependency_source_modules(
        dependency_manifest_path,
        modules,
        read_source,
        parse_source_module,
        |source_module, dependency_source| {
            collect_dependency_module_public_method_forwarders(
                dependency_package,
                source_module,
                dependency_source,
                required_types_by_module_path,
                discovered_required_types,
                forwarders,
            );
            Ok(())
        },
    )
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
#[path = "dependency_bridge_public_method_forwarders_tests.rs"]
mod tests;
