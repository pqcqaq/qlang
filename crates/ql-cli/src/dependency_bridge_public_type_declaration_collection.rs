use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use ql_ast::{ItemKind, Module, Visibility};

use crate::dependency_bridge_imports::{ImportedDependencyExterns, dependency_extern_is_imported};
use crate::dependency_bridge_modules::dependency_interface_module_import_path;
use crate::dependency_bridge_names::{
    DependencyExternOwner, record_dependency_extern_declaration, span_text,
};
use crate::dependency_bridge_public_type_errors::DependencyPublicTypeBridgeError;
use crate::dependency_bridge_public_types::{
    dependency_public_type_bridge_candidates, dependency_public_type_bridge_order,
};

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
#[path = "dependency_bridge_public_type_declaration_collection_tests.rs"]
mod tests;
