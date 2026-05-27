use std::collections::BTreeMap;
use std::path::Path;

use ql_ast::{ItemKind, Module, Visibility};

use crate::dependency_bridge_extern_errors::DependencyExternBridgeError;
use crate::dependency_bridge_imports::{ImportedDependencyExterns, dependency_extern_is_imported};
use crate::dependency_bridge_modules::dependency_interface_module_import_path;
use crate::dependency_bridge_names::{
    DependencyExternOwner, record_dependency_extern_declaration, span_text,
};

pub(crate) fn collect_dependency_module_extern_declarations(
    dependency_package: &str,
    dependency_manifest_path: &Path,
    module: &Module,
    contents: &str,
    imported_externs: Option<&ImportedDependencyExterns>,
    owners_by_symbol: &mut BTreeMap<String, DependencyExternOwner>,
    declarations: &mut Vec<String>,
) -> Result<(), DependencyExternBridgeError> {
    let module_import_path = dependency_interface_module_import_path(dependency_package, module);

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
                )
                .map_err(|(symbol, owner)| {
                    DependencyExternBridgeError::DependencyConflict { symbol, owner }
                })?;
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
                    )
                    .map_err(|(symbol, owner)| {
                        DependencyExternBridgeError::DependencyConflict { symbol, owner }
                    })?;
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

        assert!(matches!(
            error,
            DependencyExternBridgeError::DependencyConflict { symbol, owner }
                if symbol == "q_add" && owner.package_name == "first"
        ));
        assert_eq!(declarations.len(), 1);
    }
}
