use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use ql_ast::Module;

use crate::dependency_bridge_modules::{
    dependency_interface_module_import_path, dependency_module_source_path,
};
use crate::dependency_bridge_names::render_imported_dependency_public_method_forwarder;
use crate::dependency_bridge_public_types::{
    collect_dependency_public_function_type_dependencies,
    dependency_public_struct_method_bridge_candidates, dependency_public_type_bridge_candidates,
};

pub(crate) fn collect_dependency_public_method_forwarders_from_modules<E>(
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
    use ql_parser::parse_source;

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
