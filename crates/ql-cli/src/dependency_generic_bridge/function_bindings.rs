use std::collections::{BTreeMap, BTreeSet};

use ql_ast::{FunctionDecl, ItemKind, Module};

pub(super) type FunctionTypeBindings = BTreeMap<String, FunctionDecl>;

pub(super) fn collect_imported_function_type_bindings(
    root_module: &Module,
    module_import_path: &[String],
    dependency_module: &Module,
) -> FunctionTypeBindings {
    let mut bindings = FunctionTypeBindings::new();
    for item in &dependency_module.items {
        let ItemKind::Function(function) = &item.kind else {
            continue;
        };
        for local_name in
            dependency_imported_local_names(root_module, module_import_path, function.name.as_str())
        {
            bindings.insert(local_name, function.clone());
        }
    }
    bindings
}

pub(super) fn collect_local_function_type_bindings(root_module: &Module) -> FunctionTypeBindings {
    let mut bindings = FunctionTypeBindings::new();
    for item in &root_module.items {
        let ItemKind::Function(function) = &item.kind else {
            continue;
        };
        bindings.insert(function.name.clone(), function.clone());
    }
    bindings
}

pub(super) fn dependency_imported_local_names(
    root_module: &Module,
    module_import_path: &[String],
    symbol_name: &str,
) -> BTreeSet<String> {
    let mut local_names = BTreeSet::new();
    let mut full_symbol_path = module_import_path.to_vec();
    full_symbol_path.push(symbol_name.to_owned());

    for use_decl in &root_module.uses {
        if let Some(group) = &use_decl.group {
            if use_decl.prefix.segments != module_import_path {
                continue;
            }
            for item in group {
                if item.name == symbol_name {
                    local_names.insert(item.alias.clone().unwrap_or_else(|| item.name.clone()));
                }
            }
            continue;
        }

        if use_decl.prefix.segments == full_symbol_path {
            local_names.insert(
                use_decl
                    .alias
                    .clone()
                    .unwrap_or_else(|| symbol_name.to_owned()),
            );
        } else if use_decl.prefix.segments == module_import_path {
            local_names.insert(symbol_name.to_owned());
        }
    }

    local_names
}

#[cfg(test)]
#[path = "function_bindings_tests.rs"]
mod tests;
