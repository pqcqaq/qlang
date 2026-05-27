use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use ql_parser::parse_source;

use super::*;
use crate::dependency_bridge_imports::collect_top_level_definition_names;

#[test]
fn module_function_forwarders_report_local_conflicts() {
    let dependency_source = "pub fn add(value: Int) -> Int { return value + 1 }\n";
    let dependency_module = parse_source(dependency_source).unwrap();
    let root_source = "fn add(value: Int) -> Int { return value }\n";
    let root_module = parse_source(root_source).unwrap();
    let occupied_root_names = collect_top_level_definition_names(&root_module);
    let mut owners_by_symbol = BTreeMap::<String, DependencyExternOwner>::new();
    let mut required_types_by_module_path = BTreeMap::<Vec<String>, BTreeSet<String>>::new();
    let mut forwarders = Vec::new();
    let mut source_rewrites = Vec::new();
    let mut rendered_specializations = BTreeSet::new();

    let error = collect_dependency_module_public_function_forwarders(
        "dep",
        &PathBuf::from("dep/qlang.toml"),
        &dependency_module,
        dependency_source,
        &root_module,
        None,
        None,
        &occupied_root_names,
        &[],
        &mut required_types_by_module_path,
        &mut owners_by_symbol,
        &mut forwarders,
        &mut source_rewrites,
        &mut rendered_specializations,
    )
    .unwrap_err();

    assert!(matches!(
        error,
        DependencyPublicFunctionForwarderError::LocalConflict { symbol } if symbol == "add"
    ));
    assert!(forwarders.is_empty());
    assert!(source_rewrites.is_empty());
    assert!(required_types_by_module_path.is_empty());
}

#[test]
fn module_function_forwarders_report_dependency_conflicts() {
    let dependency_source = "pub fn add(value: Int) -> Int { return value + 1 }\n";
    let dependency_module = parse_source(dependency_source).unwrap();
    let root_source = "";
    let root_module = parse_source(root_source).unwrap();
    let occupied_root_names = BTreeSet::new();
    let mut owners_by_symbol = BTreeMap::<String, DependencyExternOwner>::new();
    let mut required_types_by_module_path = BTreeMap::<Vec<String>, BTreeSet<String>>::new();
    let mut forwarders = Vec::new();
    let mut source_rewrites = Vec::new();
    let mut rendered_specializations = BTreeSet::new();

    let first_result = collect_dependency_module_public_function_forwarders(
        "first",
        &PathBuf::from("first/qlang.toml"),
        &dependency_module,
        dependency_source,
        &root_module,
        None,
        None,
        &occupied_root_names,
        &[],
        &mut required_types_by_module_path,
        &mut owners_by_symbol,
        &mut forwarders,
        &mut source_rewrites,
        &mut rendered_specializations,
    );
    assert!(first_result.is_ok());
    let error = collect_dependency_module_public_function_forwarders(
        "second",
        &PathBuf::from("second/qlang.toml"),
        &dependency_module,
        dependency_source,
        &root_module,
        None,
        None,
        &occupied_root_names,
        &[],
        &mut required_types_by_module_path,
        &mut owners_by_symbol,
        &mut forwarders,
        &mut source_rewrites,
        &mut rendered_specializations,
    )
    .unwrap_err();

    assert!(matches!(
        error,
        DependencyPublicFunctionForwarderError::DependencyConflict { symbol, owner }
            if symbol == "add"
                && owner.package_name == "first"
                && owner.manifest_path == PathBuf::from("first/qlang.toml")
    ));
    assert_eq!(forwarders.len(), 1);
}
