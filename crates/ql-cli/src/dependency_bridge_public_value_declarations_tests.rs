use std::collections::{BTreeMap, BTreeSet};
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
    let mut required_functions_by_module_path = BTreeMap::<Vec<String>, BTreeSet<String>>::new();
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
    let mut required_functions_by_module_path = BTreeMap::<Vec<String>, BTreeSet<String>>::new();
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
