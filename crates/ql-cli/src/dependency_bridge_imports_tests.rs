use std::collections::BTreeSet;

use ql_parser::parse_source;

use super::*;

fn module_paths(paths: &[&[&str]]) -> BTreeSet<Vec<String>> {
    paths
        .iter()
        .map(|path| path.iter().map(|segment| (*segment).to_owned()).collect())
        .collect()
}

#[test]
fn grouped_dependency_imports_track_original_symbol_names() {
    let module = parse_source("use dep.{VALUE as LIMIT, apply as run}\n").unwrap();
    let imported = collect_imported_dependency_externs(&module, &module_paths(&[&["dep"]]));

    assert!(dependency_extern_is_imported(
        &imported,
        &["dep".to_owned()],
        "VALUE"
    ));
    assert!(dependency_extern_is_imported(
        &imported,
        &["dep".to_owned()],
        "apply"
    ));
    assert!(!dependency_extern_is_imported(
        &imported,
        &["dep".to_owned()],
        "other"
    ));
}

#[test]
fn whole_dependency_imports_cover_nested_module_paths() {
    let module = parse_source("use dep.shared\n").unwrap();
    let imported =
        collect_imported_dependency_externs(&module, &module_paths(&[&["dep", "shared", "alpha"]]));

    assert!(dependency_extern_is_imported(
        &imported,
        &["dep".to_owned(), "shared".to_owned(), "alpha".to_owned()],
        "value"
    ));
}
