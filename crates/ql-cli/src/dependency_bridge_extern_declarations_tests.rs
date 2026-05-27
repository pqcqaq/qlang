use std::collections::BTreeMap;
use std::path::PathBuf;

use ql_parser::parse_source;

use super::*;
use crate::dependency_bridge_names::DependencyExternOwner;

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
