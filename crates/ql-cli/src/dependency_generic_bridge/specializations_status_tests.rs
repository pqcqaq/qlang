use std::collections::BTreeSet;

use super::specializations_test_support::{function, parse_module};
use super::{
    PublicFunctionSpecializationRender, render_public_function_specialization_status_with_context,
};

#[test]
fn public_specialization_reports_not_called_for_unused_imported_generic() {
    let dependency_source = r#"
package dep

pub fn identity[T](value: T) -> T {
    return value
}
"#;
    let dependency = parse_module(dependency_source);
    let root = parse_module(
        r#"
use dep.identity as identity

fn main() -> Int {
    return 0
}
"#,
    );

    let status = render_public_function_specialization_status_with_context(
        &["dep".to_owned()],
        function(&dependency, "identity"),
        dependency_source,
        &root,
        &dependency,
        &[],
        &mut BTreeSet::new(),
    );

    assert_eq!(status, PublicFunctionSpecializationRender::NotCalled);
}

#[test]
fn public_specialization_reports_unsupported_for_incomplete_substitutions() {
    let dependency_source = r#"
package dep

pub fn make[T]() -> T {
    return 0
}
"#;
    let dependency = parse_module(dependency_source);
    let root = parse_module(
        r#"
use dep.make as make

fn main() -> Int {
    make()
    return 0
}
"#,
    );

    let status = render_public_function_specialization_status_with_context(
        &["dep".to_owned()],
        function(&dependency, "make"),
        dependency_source,
        &root,
        &dependency,
        &[],
        &mut BTreeSet::new(),
    );

    assert_eq!(status, PublicFunctionSpecializationRender::Unsupported);
}
