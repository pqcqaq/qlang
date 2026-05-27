use std::path::Path;

use ql_ast::Module;
use ql_project::InterfaceModule;

use crate::dependency_bridge_imports::{
    ImportedDependencyExterns, collect_imported_dependency_externs,
};
use crate::dependency_bridge_modules::{
    dependency_interface_module_import_path, dependency_interface_module_import_paths,
    dependency_module_source_path,
};

pub(crate) fn collect_dependency_source_modules<E>(
    dependency_manifest_path: &Path,
    interface_modules: &[InterfaceModule],
    mut read_source: impl FnMut(&Path) -> Result<String, E>,
    mut parse_source_module: impl FnMut(&Path, &str) -> Result<Module, E>,
    mut collect_source_module: impl FnMut(&Module, &str) -> Result<(), E>,
) -> Result<(), E> {
    for module in interface_modules {
        let dependency_source_path =
            dependency_module_source_path(dependency_manifest_path, &module.source_path);
        let dependency_source = read_source(&dependency_source_path)?;
        let source_module = parse_source_module(&dependency_source_path, &dependency_source)?;
        collect_source_module(&source_module, &dependency_source)?;
    }

    Ok(())
}

pub(crate) fn collect_dependency_source_module_bridge_items<E>(
    dependency_package: &str,
    dependency_manifest_path: &Path,
    interface_modules: &[InterfaceModule],
    root_source_module: &Module,
    read_source: impl FnMut(&Path) -> Result<String, E>,
    parse_source_module: impl FnMut(&Path, &str) -> Result<Module, E>,
    mut collect_source_module: impl FnMut(
        &Module,
        &str,
        &ImportedDependencyExterns,
        &Vec<String>,
    ) -> Result<(), E>,
) -> Result<(), E> {
    let module_import_paths =
        dependency_interface_module_import_paths(dependency_package, interface_modules);
    let imported_externs =
        collect_imported_dependency_externs(root_source_module, &module_import_paths);

    collect_dependency_source_modules(
        dependency_manifest_path,
        interface_modules,
        read_source,
        parse_source_module,
        |source_module, dependency_source| {
            let module_import_path =
                dependency_interface_module_import_path(dependency_package, source_module);
            collect_source_module(
                source_module,
                dependency_source,
                &imported_externs,
                &module_import_path,
            )
        },
    )
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use ql_parser::parse_source;

    use super::*;
    use crate::dependency_bridge_imports::dependency_extern_is_imported;

    fn interface_module(source_path: &str, source: &str) -> InterfaceModule {
        InterfaceModule {
            source_path: source_path.to_owned(),
            contents: source.to_owned(),
            syntax: parse_source(source).unwrap(),
        }
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
}
