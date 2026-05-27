use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use ql_parser::parse_source;
use ql_project::{InterfaceModule, PackageManifest, ProjectManifest, ReferencesManifest};

use super::*;
use crate::dependency_bridge_imports::dependency_extern_is_imported;

fn interface_module(source_path: &str, source: &str) -> InterfaceModule {
    InterfaceModule {
        source_path: source_path.to_owned(),
        contents: source.to_owned(),
        syntax: parse_source(source).unwrap(),
    }
}

fn package_manifest(path: PathBuf, name: &str) -> ProjectManifest {
    ProjectManifest {
        manifest_path: path,
        package: Some(PackageManifest {
            name: name.to_owned(),
        }),
        workspace: None,
        references: ReferencesManifest::default(),
        profile: None,
        lib: None,
        bins: Vec::new(),
    }
}

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("{prefix}-{}-{unique}", std::process::id()));
    fs::create_dir_all(&path).unwrap();
    path
}

#[test]
fn interface_module_bridge_items_share_import_filtering_context() {
    let first_source = "package dep.alpha\npub const FIRST: Int = 1\n";
    let second_source = "package dep.beta\npub const SECOND: Int = 2\n";
    let interface_modules = [
        interface_module("src/alpha.ql", first_source),
        interface_module("src/beta.ql", second_source),
    ];
    let root_source_module = parse_source("use dep.alpha.FIRST\n").unwrap();
    let mut visited = Vec::new();

    collect_dependency_interface_module_bridge_items(
        "dep",
        &interface_modules,
        &root_source_module,
        |module, imported_externs, module_import_path| {
            visited.push((module.source_path.clone(), module_import_path.clone()));
            let symbol = module
                .syntax
                .items
                .first()
                .and_then(|item| match &item.kind {
                    ql_ast::ItemKind::Const(global) => Some(global.name.as_str()),
                    _ => None,
                });
            if let Some(symbol) = symbol {
                let imported =
                    dependency_extern_is_imported(imported_externs, module_import_path, symbol);
                assert_eq!(imported, symbol == "FIRST");
            }
            Ok::<_, String>(())
        },
    )
    .unwrap();

    assert_eq!(
        visited,
        vec![
            (
                "src/alpha.ql".to_owned(),
                vec!["dep".to_owned(), "alpha".to_owned()]
            ),
            (
                "src/beta.ql".to_owned(),
                vec!["dep".to_owned(), "beta".to_owned()]
            ),
        ]
    );
}

#[test]
fn source_modules_read_parse_and_collect_in_interface_order() {
    let first_source = "pub const FIRST: Int = 1\n";
    let second_source = "pub const SECOND: Int = 2\n";
    let interface_modules = [
        interface_module("src/first.ql", first_source),
        interface_module("src/second.ql", second_source),
    ];
    let mut visited = Vec::new();

    collect_dependency_source_modules(
        &PathBuf::from("dep/qlang.toml"),
        &interface_modules,
        |dependency_source_path| match dependency_source_path
            .to_string_lossy()
            .replace('\\', "/")
            .as_str()
        {
            "dep/src/first.ql" => Ok::<_, String>(first_source.to_owned()),
            "dep/src/second.ql" => Ok(second_source.to_owned()),
            other => Err(format!("unexpected path: {other}")),
        },
        |_dependency_source_path, source| Ok(parse_source(source).unwrap()),
        |_module, source| {
            visited.push(source.to_owned());
            Ok(())
        },
    )
    .unwrap();

    assert_eq!(
        visited,
        vec![first_source.to_owned(), second_source.to_owned()]
    );
}

#[test]
fn source_module_bridge_items_share_import_filtering_context() {
    let dependency_source = "pub const VALUE: Int = 1\npub const OTHER: Int = 2\n";
    let interface_modules = [interface_module("src/lib.ql", dependency_source)];
    let root_source_module = parse_source("use dep.VALUE as imported\n").unwrap();
    let mut visited = Vec::new();

    collect_dependency_source_module_bridge_items(
        "dep",
        &PathBuf::from("dep/qlang.toml"),
        &interface_modules,
        &root_source_module,
        |dependency_source_path| {
            assert_eq!(dependency_source_path, Path::new("dep/src/lib.ql"));
            Ok::<_, String>(dependency_source.to_owned())
        },
        |_dependency_source_path, source| Ok(parse_source(source).unwrap()),
        |_module, source, imported_externs, module_import_path| {
            assert_eq!(source, dependency_source);
            assert_eq!(module_import_path, &vec!["dep".to_owned()]);
            assert!(dependency_extern_is_imported(
                imported_externs,
                module_import_path,
                "VALUE"
            ));
            assert!(!dependency_extern_is_imported(
                imported_externs,
                module_import_path,
                "OTHER"
            ));
            visited.push(module_import_path.clone());
            Ok(())
        },
    )
    .unwrap();

    assert_eq!(visited, vec![vec!["dep".to_owned()]]);
}

#[test]
fn source_module_bridge_items_propagate_parse_errors_before_collection() {
    let interface_modules = [interface_module("src/lib.ql", "pub const VALUE: Int = 1\n")];
    let root_source_module = parse_source("use dep.VALUE\n").unwrap();
    let mut collection_count = 0;

    let error = collect_dependency_source_module_bridge_items(
        "dep",
        &PathBuf::from("dep/qlang.toml"),
        &interface_modules,
        &root_source_module,
        |_dependency_source_path| Ok::<_, String>("not qlang".to_owned()),
        |_dependency_source_path, _source| Err("parse failed".to_owned()),
        |_module, _source, _imported_externs, _module_import_path| {
            collection_count += 1;
            Ok(())
        },
    )
    .unwrap_err();

    assert_eq!(error, "parse failed");
    assert_eq!(collection_count, 0);
}

#[test]
fn direct_dependency_source_module_bridge_items_load_interfaces_and_sources_in_order() {
    let temp = unique_temp_dir("ql-direct-dependency-source-module-bridge");
    let dep_root = temp.join("dep");
    let dep_src = dep_root.join("src");
    fs::create_dir_all(&dep_src).unwrap();
    fs::write(
        dep_src.join("alpha.ql"),
        "package dep.alpha\npub const FIRST: Int = 1\n",
    )
    .unwrap();
    fs::write(
        dep_src.join("beta.ql"),
        "package dep.beta\npub const SECOND: Int = 2\n",
    )
    .unwrap();
    fs::write(
        dep_root.join("dep.qi"),
        concat!(
            "// qlang interface v1\n",
            "// package: dep\n",
            "// source: src/alpha.ql\n",
            "package dep.alpha\n",
            "pub const FIRST: Int\n",
            "// source: src/beta.ql\n",
            "package dep.beta\n",
            "pub const SECOND: Int\n",
        ),
    )
    .unwrap();

    let dependency = package_manifest(dep_root.join("qlang.toml"), "dep");
    let root_source_module = parse_source("use dep.alpha.FIRST\n").unwrap();
    let mut visited = Vec::new();

    collect_direct_dependency_source_module_bridge_items(
        &[dependency],
        &root_source_module,
        |_dependency, error| format!("package: {error}"),
        |_dependency, error| format!("interface path: {error}"),
        |_dependency, _package, _interface_path, error| format!("interface: {error}"),
        |_dependency, _package, source_path| {
            fs::read_to_string(source_path).map_err(|error| error.to_string())
        },
        |_dependency, _package, _source_path, source| {
            parse_source(source).map_err(|_| "parse".to_owned())
        },
        |_dependency, package, source_module, _source, imported_externs, module_import_path| {
            let symbol = source_module
                .items
                .first()
                .and_then(|item| match &item.kind {
                    ql_ast::ItemKind::Const(global) => Some(global.name.as_str()),
                    _ => None,
                });
            if let Some(symbol) = symbol {
                let imported =
                    dependency_extern_is_imported(imported_externs, module_import_path, symbol);
                visited.push((package.to_owned(), module_import_path.clone(), imported));
            }
            Ok::<_, String>(())
        },
    )
    .unwrap();

    assert_eq!(
        visited,
        vec![
            (
                "dep".to_owned(),
                vec!["dep".to_owned(), "alpha".to_owned()],
                true
            ),
            (
                "dep".to_owned(),
                vec!["dep".to_owned(), "beta".to_owned()],
                false
            ),
        ]
    );
    let _ = fs::remove_dir_all(temp);
}
