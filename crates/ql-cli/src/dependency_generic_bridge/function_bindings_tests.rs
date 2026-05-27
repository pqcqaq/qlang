use std::collections::BTreeSet;

use ql_ast::Module;

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

    let bindings = collect_imported_function_type_bindings(&root, &["dep".to_owned()], &dependency);

    assert_eq!(
        bindings.keys().cloned().collect::<BTreeSet<_>>(),
        BTreeSet::from(["id".to_owned(), "pick".to_owned()])
    );
    assert_eq!(bindings["id"].name, "identity");
    assert_eq!(bindings["pick"].name, "choose");
}
