use super::*;

fn parse_module(source: &str) -> Module {
    ql_parser::parse_source(source).expect("test source should parse")
}

#[test]
fn body_call_targets_include_same_module_and_imported_local_names() {
    let dependency_source = r#"
package dep

use helper.identity as helper_identity
use helper.{wrap as grouped_wrap}

fn local_identity[T](value: T) -> T {
    return value
}

pub fn root[T](value: T) -> T {
    return local_identity(helper_identity(grouped_wrap(value)))
}
"#;
    let helper_source = r#"
package helper

pub fn identity[T](value: T) -> T {
    return value
}

pub fn wrap[T](value: T) -> T {
    return value
}
"#;
    let dependency = parse_module(dependency_source);
    let helper = parse_module(helper_source);
    let helper_import_path = vec!["helper".to_owned()];
    let helper_module = SpecializationModule {
        module_import_path: &helper_import_path,
        contents: helper_source,
        module: &helper,
    };
    let dependency_import_path = vec!["dep".to_owned()];

    let targets = specialized_body_call_targets(
        &dependency_import_path,
        dependency_source,
        &dependency,
        &[helper_module],
    );

    let rendered = targets
        .into_iter()
        .map(|target| {
            (
                target.module_import_path.join("."),
                target.callee.name.clone(),
                target.local_names,
            )
        })
        .collect::<Vec<_>>();

    assert!(rendered.contains(&(
        "dep".to_owned(),
        "local_identity".to_owned(),
        BTreeSet::from(["local_identity".to_owned()])
    )));
    assert!(rendered.contains(&(
        "helper".to_owned(),
        "identity".to_owned(),
        BTreeSet::from(["helper_identity".to_owned()])
    )));
    assert!(rendered.contains(&(
        "helper".to_owned(),
        "wrap".to_owned(),
        BTreeSet::from(["grouped_wrap".to_owned()])
    )));
}
