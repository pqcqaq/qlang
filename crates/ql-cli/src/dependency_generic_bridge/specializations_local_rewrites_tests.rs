use std::collections::BTreeSet;

use super::render_public_function_specializations;
use super::specializations_test_support::{function, parse_module};

#[test]
fn public_specialization_rewrites_same_dependency_generic_body_calls() {
    let dependency_source = r#"
package dep

pub fn first[T, N](values: [T; N]) -> T {
    return values[0]
}

pub fn first_wrapped[T, N](values: [T; N]) -> T {
    return first(values)
}
"#;
    let dependency = parse_module(dependency_source);
    let root = parse_module(
        r#"
use dep.first_wrapped as first_wrapped

fn main() -> Int {
    return first_wrapped([1, 2, 3])
}
"#,
    );

    let rendered = render_public_function_specializations(
        &["dep".to_owned()],
        function(&dependency, "first_wrapped"),
        dependency_source,
        &root,
        &dependency,
        &mut BTreeSet::new(),
    )
    .expect("first_wrapped should render a concrete specialization");

    assert!(
        rendered
            .declarations
            .contains("fn __ql_bridge_local_dep_first__generic_Int_3(values: [Int; 3]) -> Int")
    );
    assert!(
        rendered
            .declarations
            .contains("return __ql_bridge_local_dep_first__generic_Int_3(values)")
    );
    assert!(!rendered.declarations.contains("return first(values)"));
    assert_eq!(rendered.call_rewrites.len(), 1);
    assert_eq!(
        rendered.call_rewrites[0].replacement,
        "__ql_bridge_local_dep_first_wrapped__generic_Int_3"
    );
}

#[test]
fn public_specialization_rewrites_helper_after_typed_local_substitution() {
    let dependency_source = r#"
package dep

pub fn helper[T](value: T) -> T {
    return value
}

pub fn wrapped[T](value: T) -> T {
    let local: T = value
    return helper(local)
}
"#;
    let dependency = parse_module(dependency_source);
    let root = parse_module(
        r#"
use dep.wrapped as wrapped

fn main() -> Int {
    return wrapped(1)
}
"#,
    );

    let rendered = render_public_function_specializations(
        &["dep".to_owned()],
        function(&dependency, "wrapped"),
        dependency_source,
        &root,
        &dependency,
        &mut BTreeSet::new(),
    )
    .expect("wrapped should render a concrete specialization");

    assert!(
        rendered
            .declarations
            .contains("fn __ql_bridge_local_dep_helper__generic_Int(value: Int) -> Int")
    );
    assert!(rendered.declarations.contains("let local: Int = value"));
    assert!(
        rendered
            .declarations
            .contains("return __ql_bridge_local_dep_helper__generic_Int(local)")
    );
    assert!(!rendered.declarations.contains("__generic_T"));
    assert_eq!(rendered.call_rewrites.len(), 1);
    assert_eq!(
        rendered.call_rewrites[0].replacement,
        "__ql_bridge_local_dep_wrapped__generic_Int"
    );
}

#[test]
fn public_specialization_rewrites_helper_after_generic_carrier_pattern_binding() {
    let dependency_source = r#"
package dep

pub enum Option[T] {
    Some(T),
    None,
}

pub fn helper[T](value: T) -> T {
    return value
}

pub fn wrapped[T](value: Option[T], fallback: T) -> T {
    return match value {
        Option.Some(inner) => helper(inner),
        Option.None => fallback,
    }
}
"#;
    let dependency = parse_module(dependency_source);
    let root = parse_module(
        r#"
use dep.Option as Option
use dep.wrapped as wrapped

fn main() -> Int {
    return wrapped(Option.Some(1), 0)
}
"#,
    );

    let rendered = render_public_function_specializations(
        &["dep".to_owned()],
        function(&dependency, "wrapped"),
        dependency_source,
        &root,
        &dependency,
        &mut BTreeSet::new(),
    )
    .expect("wrapped should render a concrete specialization");

    assert!(
        rendered
            .declarations
            .contains("fn __ql_bridge_local_dep_helper__generic_Int(value: Int) -> Int")
    );
    assert!(
        rendered
            .declarations
            .contains("Option.Some(inner) => __ql_bridge_local_dep_helper__generic_Int(inner)")
    );
    assert!(!rendered.declarations.contains("__generic_T"));
    assert_eq!(rendered.call_rewrites.len(), 1);
    assert_eq!(
        rendered.call_rewrites[0].replacement,
        "__ql_bridge_local_dep_wrapped__generic_Int"
    );
}

#[test]
fn public_specialization_preserves_non_generic_body_helper_calls() {
    let dependency_source = r#"
package dep

pub fn helper(value: Int) -> Int {
    return value + 1
}

pub fn wrapped[T](value: T, count: Int) -> Int {
    return helper(count)
}
"#;
    let dependency = parse_module(dependency_source);
    let root = parse_module(
        r#"
use dep.wrapped as wrapped

fn main() -> Int {
    return wrapped(0, 1)
}
"#,
    );

    let rendered = render_public_function_specializations(
        &["dep".to_owned()],
        function(&dependency, "wrapped"),
        dependency_source,
        &root,
        &dependency,
        &mut BTreeSet::new(),
    )
    .expect("wrapped should render a concrete specialization");

    assert!(
        rendered.declarations.contains(
            "fn __ql_bridge_local_dep_wrapped__generic_Int(value: Int, count: Int) -> Int"
        )
    );
    assert!(rendered.declarations.contains("return helper(count)"));
    assert!(
        !rendered
            .declarations
            .contains("__ql_bridge_local_dep_helper")
    );
    assert_eq!(rendered.call_rewrites.len(), 1);
    assert_eq!(
        rendered.call_rewrites[0].replacement,
        "__ql_bridge_local_dep_wrapped__generic_Int"
    );
}

#[test]
fn public_specialization_preserves_unresolved_array_length_generics() {
    let dependency_source = r#"
package dep

pub fn total[N](values: [Int; N]) -> Int {
    var sum = 0
    for value in values {
        sum = sum + value
    }
    return sum
}

pub fn mirror_sum[N](values: [Int; N]) -> Int {
    return total(values)
}
"#;
    let dependency = parse_module(dependency_source);
    let root = parse_module(
        r#"
use dep.mirror_sum as mirror_sum

fn score[N](values: [Int; N]) -> Int {
    return mirror_sum(values)
}
"#,
    );

    let rendered = render_public_function_specializations(
        &["dep".to_owned()],
        function(&dependency, "mirror_sum"),
        dependency_source,
        &root,
        &dependency,
        &mut BTreeSet::new(),
    )
    .expect("mirror_sum should render a length-generic specialization");

    assert!(
        rendered
            .declarations
            .contains("fn __ql_bridge_local_dep_mirror_sum__generic_N[N](values: [Int; N]) -> Int")
    );
    assert!(
        rendered
            .declarations
            .contains("fn __ql_bridge_local_dep_total__generic_N[N](values: [Int; N]) -> Int")
    );
    assert!(
        rendered
            .declarations
            .contains("return __ql_bridge_local_dep_total__generic_N(values)")
    );
    assert_eq!(rendered.call_rewrites.len(), 1);
    assert_eq!(
        rendered.call_rewrites[0].replacement,
        "__ql_bridge_local_dep_mirror_sum__generic_N"
    );
}
