use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use ql_ast::{ItemKind, Module, Visibility};

use crate::dependency_bridge_imports::{ImportedDependencyExterns, dependency_extern_is_imported};
use crate::dependency_bridge_modules::dependency_interface_module_import_path;
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
use crate::dependency_bridge_public_value_errors::DependencyPublicValueBridgeError;

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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use ql_parser::parse_source;

    use super::*;
    use crate::dependency_bridge_imports::collect_top_level_definition_names;

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
