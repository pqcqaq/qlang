use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use ql_ast::Module;

use crate::dependency_bridge_imports::{
    collect_imported_dependency_externs, collect_top_level_definition_names,
};
use crate::dependency_bridge_modules::{
    PackageBridgeModule, dependency_interface_module_import_path,
};
use crate::dependency_bridge_names::DependencyExternOwner;
use crate::dependency_bridge_public_function_errors::DependencyPublicFunctionForwarderError;
use crate::dependency_bridge_public_function_forwarders::collect_dependency_module_public_function_forwarders;
use crate::dependency_bridge_public_type_declaration_collection::collect_dependency_module_public_type_declarations;
use crate::dependency_bridge_public_type_errors::DependencyPublicTypeBridgeError;
use crate::dependency_generic_bridge;

pub(crate) struct CollectedPackageUnderTestBridgeItems {
    pub(crate) function_forwarders: Vec<String>,
    pub(crate) type_declarations: Vec<String>,
    pub(crate) source_rewrites: Vec<dependency_generic_bridge::SourceRewrite>,
}

pub(crate) enum PackageUnderTestBridgeCollectionError {
    Function(DependencyPublicFunctionForwarderError),
    Type(DependencyPublicTypeBridgeError),
}

pub(crate) fn collect_package_under_test_bridge_items(
    package_name: &str,
    manifest_path: &Path,
    root_source_module: &Module,
    bridge_modules: &[PackageBridgeModule],
    specialization_modules: &[dependency_generic_bridge::SpecializationModule<'_>],
) -> Result<CollectedPackageUnderTestBridgeItems, PackageUnderTestBridgeCollectionError> {
    let module_import_paths = bridge_modules
        .iter()
        .map(|module| dependency_interface_module_import_path(package_name, &module.module))
        .collect::<BTreeSet<_>>();
    let imported_externs =
        collect_imported_dependency_externs(root_source_module, &module_import_paths);
    let occupied_root_names = collect_top_level_definition_names(root_source_module);

    let mut function_forwarders = Vec::new();
    let mut source_rewrites = Vec::new();
    let mut required_types_by_module_path = BTreeMap::<Vec<String>, BTreeSet<String>>::new();
    let mut function_owners = BTreeMap::<String, DependencyExternOwner>::new();
    let mut rendered_specializations = BTreeSet::new();
    for module in bridge_modules {
        collect_dependency_module_public_function_forwarders(
            package_name,
            manifest_path,
            &module.module,
            &module.source,
            root_source_module,
            Some(&imported_externs),
            None,
            &occupied_root_names,
            specialization_modules,
            &mut required_types_by_module_path,
            &mut function_owners,
            &mut function_forwarders,
            &mut source_rewrites,
            &mut rendered_specializations,
        )
        .map_err(PackageUnderTestBridgeCollectionError::Function)?;
    }

    let mut type_declarations = Vec::new();
    let mut type_owners = BTreeMap::<String, DependencyExternOwner>::new();
    for module in bridge_modules {
        let module_import_path =
            dependency_interface_module_import_path(package_name, &module.module);
        collect_dependency_module_public_type_declarations(
            package_name,
            manifest_path,
            &module.module,
            &module.source,
            Some(&imported_externs),
            required_types_by_module_path.get(&module_import_path),
            &occupied_root_names,
            &mut type_owners,
            &mut type_declarations,
        )
        .map_err(PackageUnderTestBridgeCollectionError::Type)?;
    }

    Ok(CollectedPackageUnderTestBridgeItems {
        function_forwarders,
        type_declarations,
        source_rewrites,
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use ql_parser::parse_source;

    use super::*;

    #[test]
    fn package_under_test_collection_includes_function_type_dependencies() {
        let package_source =
            "pub struct Box { value: Int }\npub fn open(box: Box) -> Int { return box.value }\n";
        let root_source_module = parse_source(
            "use app.open as open\n\nfn main() -> Int { return open(Box { value: 1 }) }\n",
        )
        .unwrap();
        let bridge_modules = [PackageBridgeModule {
            source: package_source.to_owned(),
            module: parse_source(package_source).unwrap(),
        }];

        let collected = match collect_package_under_test_bridge_items(
            "app",
            &PathBuf::from("app/qlang.toml"),
            &root_source_module,
            &bridge_modules,
            &[],
        ) {
            Ok(collected) => collected,
            Err(_) => panic!("package-under-test bridge collection should succeed"),
        };

        assert_eq!(collected.source_rewrites.len(), 0);
        assert_eq!(collected.function_forwarders.len(), 1);
        assert_eq!(collected.type_declarations.len(), 1);
        assert!(collected.function_forwarders[0].contains("const open: (Box) -> Int"));
        assert!(collected.type_declarations[0].contains("pub struct Box"));
    }
}
