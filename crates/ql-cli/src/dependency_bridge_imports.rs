use std::collections::{BTreeMap, BTreeSet};

use ql_ast::{ItemKind, Module};

#[derive(Clone, Debug, Default)]
pub(crate) struct ImportedDependencyExterns {
    whole_paths: BTreeSet<Vec<String>>,
    symbols_by_module_path: BTreeMap<Vec<String>, BTreeSet<String>>,
}

pub(crate) fn collect_top_level_definition_names(module: &Module) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for item in &module.items {
        match &item.kind {
            ItemKind::Function(function) => {
                names.insert(function.name.clone());
            }
            ItemKind::Const(global) | ItemKind::Static(global) => {
                names.insert(global.name.clone());
            }
            ItemKind::Struct(struct_decl) => {
                names.insert(struct_decl.name.clone());
            }
            ItemKind::Enum(enum_decl) => {
                names.insert(enum_decl.name.clone());
            }
            ItemKind::Trait(trait_decl) => {
                names.insert(trait_decl.name.clone());
            }
            ItemKind::TypeAlias(alias) => {
                names.insert(alias.name.clone());
            }
            ItemKind::ExternBlock(extern_block) => {
                for function in &extern_block.functions {
                    names.insert(function.name.clone());
                }
            }
            ItemKind::Impl(_) | ItemKind::Extend(_) => {}
        }
    }
    names
}

pub(crate) fn collect_imported_dependency_externs(
    root_module: &Module,
    module_paths: &BTreeSet<Vec<String>>,
) -> ImportedDependencyExterns {
    let mut imported = ImportedDependencyExterns::default();

    for use_decl in &root_module.uses {
        if let Some(group) = &use_decl.group {
            if !module_paths.contains(&use_decl.prefix.segments) {
                continue;
            }
            let symbols = imported
                .symbols_by_module_path
                .entry(use_decl.prefix.segments.clone())
                .or_default();
            for item in group {
                symbols.insert(item.name.clone());
            }
            continue;
        }

        if module_paths.contains(&use_decl.prefix.segments)
            || module_paths
                .iter()
                .any(|module_path| module_path.starts_with(&use_decl.prefix.segments))
        {
            imported
                .whole_paths
                .insert(use_decl.prefix.segments.clone());
            continue;
        }

        if use_decl.prefix.segments.len() < 2 {
            continue;
        }

        let module_path = use_decl.prefix.segments[..use_decl.prefix.segments.len() - 1].to_vec();
        if !module_paths.contains(&module_path) {
            continue;
        }

        if let Some(symbol_name) = use_decl.prefix.segments.last() {
            imported
                .symbols_by_module_path
                .entry(module_path)
                .or_default()
                .insert(symbol_name.clone());
        }
    }

    imported
}

pub(crate) fn dependency_extern_is_imported(
    imported_externs: &ImportedDependencyExterns,
    module_path: &[String],
    symbol_name: &str,
) -> bool {
    imported_externs
        .whole_paths
        .iter()
        .any(|whole_path| module_path.starts_with(whole_path))
        || imported_externs
            .symbols_by_module_path
            .get(module_path)
            .is_some_and(|symbols| symbols.contains(symbol_name))
}

pub(crate) fn extend_dependency_bridge_name_requirements(
    destination: &mut BTreeMap<Vec<String>, BTreeSet<String>>,
    source: &BTreeMap<Vec<String>, BTreeSet<String>>,
) {
    for (module_path, symbols) in source {
        destination
            .entry(module_path.clone())
            .or_default()
            .extend(symbols.iter().cloned());
    }
}

#[cfg(test)]
#[path = "dependency_bridge_imports_tests.rs"]
mod tests;
