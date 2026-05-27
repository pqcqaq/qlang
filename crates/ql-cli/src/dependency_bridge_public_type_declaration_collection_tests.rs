use std::collections::BTreeMap;
use std::path::PathBuf;

use ql_parser::parse_source;

use super::*;
use crate::dependency_bridge_imports::collect_top_level_definition_names;

#[test]
fn module_type_declarations_report_local_conflicts() {
    let dependency_source = "pub struct Box { value: Int }\n";
    let dependency_module = parse_source(dependency_source).unwrap();
    let root_source = "struct Box { value: Int }\n";
    let root_module = parse_source(root_source).unwrap();
    let occupied_root_names = collect_top_level_definition_names(&root_module);
    let mut owners_by_symbol = BTreeMap::<String, DependencyExternOwner>::new();
    let mut declarations = Vec::new();

    let error = collect_dependency_module_public_type_declarations(
        "dep",
        &PathBuf::from("dep/qlang.toml"),
        &dependency_module,
        dependency_source,
        None,
        None,
        &occupied_root_names,
        &mut owners_by_symbol,
        &mut declarations,
    )
    .unwrap_err();

    assert!(matches!(
        error,
        DependencyPublicTypeBridgeError::LocalConflict { symbol } if symbol == "Box"
    ));
    assert!(declarations.is_empty());
}
