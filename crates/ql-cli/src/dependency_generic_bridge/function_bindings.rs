use std::collections::{BTreeMap, BTreeSet};

use ql_ast::{FunctionDecl, ItemKind, Module};

pub(super) type FunctionTypeBindings = BTreeMap<String, FunctionDecl>;

pub(super) fn collect_imported_function_type_bindings(
    root_module: &Module,
    module_import_path: &[String],
    dependency_module: &Module,
) -> FunctionTypeBindings {
    let mut bindings = FunctionTypeBindings::new();
    for item in &dependency_module.items {
        let ItemKind::Function(function) = &item.kind else {
            continue;
        };
        for local_name in
            dependency_imported_local_names(root_module, module_import_path, function.name.as_str())
        {
            bindings.insert(local_name, function.clone());
        }
    }
    bindings
}

pub(super) fn collect_local_function_type_bindings(root_module: &Module) -> FunctionTypeBindings {
    let mut bindings = FunctionTypeBindings::new();
    for item in &root_module.items {
        let ItemKind::Function(function) = &item.kind else {
            continue;
        };
        bindings.insert(function.name.clone(), function.clone());
    }
    bindings
}

pub(super) fn dependency_imported_local_names(
    root_module: &Module,
    module_import_path: &[String],
    symbol_name: &str,
) -> BTreeSet<String> {
    let mut local_names = BTreeSet::new();
    let mut full_symbol_path = module_import_path.to_vec();
    full_symbol_path.push(symbol_name.to_owned());

    for use_decl in &root_module.uses {
        if let Some(group) = &use_decl.group {
            if use_decl.prefix.segments != module_import_path {
                continue;
            }
            for item in group {
                if item.name == symbol_name {
                    local_names.insert(item.alias.clone().unwrap_or_else(|| item.name.clone()));
                }
            }
            continue;
        }

        if use_decl.prefix.segments == full_symbol_path {
            local_names.insert(
                use_decl
                    .alias
                    .clone()
                    .unwrap_or_else(|| symbol_name.to_owned()),
            );
        } else if use_decl.prefix.segments == module_import_path {
            local_names.insert(symbol_name.to_owned());
        }
    }

    local_names
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_module(source: &str) -> Module {
        ql_parser::parse_source(source).expect("test source should parse")
    }

    #[test]
    fn imported_local_names_include_alias_group_and_module_imports() {
        let root = parse_module(
            r#"
use dep.identity as alias_identity
use dep.{choose as pick, identity as grouped_identity}
use dep

fn run() -> Int { return 0 }
"#,
        );
        let local_names = dependency_imported_local_names(&root, &["dep".to_owned()], "identity");

        assert_eq!(
            local_names,
            BTreeSet::from([
                "alias_identity".to_owned(),
                "grouped_identity".to_owned(),
                "identity".to_owned(),
            ])
        );
    }

    #[test]
    fn imported_function_bindings_use_imported_local_names() {
        let root = parse_module(
            r#"
use dep.identity as id
use dep.{choose as pick}

fn run() -> Int { return 0 }
"#,
        );
        let dependency = parse_module(
            r#"
package dep

pub fn identity[T](value: T) -> T { return value }
pub fn choose[T](fallback: T, value: T) -> T { return value }
"#,
        );

        let bindings =
            collect_imported_function_type_bindings(&root, &["dep".to_owned()], &dependency);

        assert_eq!(
            bindings.keys().cloned().collect::<BTreeSet<_>>(),
            BTreeSet::from(["id".to_owned(), "pick".to_owned()])
        );
        assert_eq!(bindings["id"].name, "identity");
        assert_eq!(bindings["pick"].name, "choose");
    }
}
