use std::collections::BTreeSet;

use super::specializations_test_support::{function, parse_module};
use super::{
    PublicFunctionSpecializationRender, SpecializationModule,
    render_public_function_specialization_status_with_context,
};

#[test]
fn public_specialization_uses_imported_enum_bindings_from_specialization_modules() {
    let option_source = r#"
package std.option

pub enum Option[T] {
    Some(T),
    None,
}
"#;
    let dependency_source = r#"
package std.test

use std.option.Option as Option

pub fn expect_option_none[T](value: Option[T]) -> Int {
    return match value {
        Option.Some(_) => 1,
        Option.None => 0,
    }
}
"#;
    let option = parse_module(option_source);
    let dependency = parse_module(dependency_source);
    let root = parse_module(
        r#"
use std.option.Option as Option
use std.test.expect_option_none as expect_option_none

fn run() -> Int {
    let missing: Option[String] = Option.None
    return expect_option_none(missing) + expect_option_none(Option.Some(7))
}
"#,
    );
    let option_import_path = vec!["std".to_owned(), "option".to_owned()];
    let option_module = SpecializationModule {
        module_import_path: &option_import_path,
        contents: option_source,
        module: &option,
    };

    let rendered = render_public_function_specialization_status_with_context(
        &["std".to_owned(), "test".to_owned()],
        function(&dependency, "expect_option_none"),
        dependency_source,
        &root,
        &dependency,
        &[option_module],
        &mut BTreeSet::new(),
    );
    let PublicFunctionSpecializationRender::Rendered(rendered) = rendered else {
        panic!("expect_option_none should render with imported Option bindings");
    };

    assert!(rendered.declarations.contains(
        "fn __ql_bridge_local_std_test_expect_option_none__generic_String(value: Option[String]) -> Int"
    ));
    assert!(rendered.declarations.contains(
        "fn __ql_bridge_local_std_test_expect_option_none__generic_Int(value: Option[Int]) -> Int"
    ));
    assert_eq!(rendered.call_rewrites.len(), 2);
}

#[test]
fn public_specialization_rewrites_imported_dependency_generic_body_calls() {
    let helper_source = r#"
package helper

pub fn reverse_array[T, N](values: [T; N]) -> [T; N] {
    return values
}
"#;
    let dependency_source = r#"
package dep

use helper.reverse_array as rev

pub fn reverse_wrapped[T, N](values: [T; N]) -> [T; N] {
    return rev(values)
}
"#;
    let helper = parse_module(helper_source);
    let dependency = parse_module(dependency_source);
    let root = parse_module(
        r#"
use dep.reverse_wrapped as reverse_wrapped

fn run() -> [String; 3] {
    return reverse_wrapped(["north", "east", "south"])
}
"#,
    );
    let helper_import_path = vec!["helper".to_owned()];
    let helper_module = SpecializationModule {
        module_import_path: &helper_import_path,
        contents: helper_source,
        module: &helper,
    };

    let rendered = render_public_function_specialization_status_with_context(
        &["dep".to_owned()],
        function(&dependency, "reverse_wrapped"),
        dependency_source,
        &root,
        &dependency,
        &[helper_module],
        &mut BTreeSet::new(),
    );
    let PublicFunctionSpecializationRender::Rendered(rendered) = rendered else {
        panic!("reverse_wrapped should render imported helper specialization");
    };

    assert!(rendered.declarations.contains(
        "fn __ql_bridge_local_helper_reverse_array__generic_String_3(values: [String; 3]) -> [String; 3]"
    ));
    assert!(
        rendered
            .declarations
            .contains("return __ql_bridge_local_helper_reverse_array__generic_String_3(values)")
    );
    assert!(!rendered.declarations.contains("return rev(values)"));
    assert_eq!(rendered.call_rewrites.len(), 1);
    assert_eq!(
        rendered.call_rewrites[0].replacement,
        "__ql_bridge_local_dep_reverse_wrapped__generic_String_3"
    );
}

#[test]
fn public_specialization_rewrites_grouped_imported_dependency_generic_body_calls() {
    let helper_source = r#"
package helper

pub fn wrap_value[T](value: T) -> T {
    return value
}
"#;
    let dependency_source = r#"
package dep

use helper.{wrap_value as wrap}

pub fn wrapped[T](value: T) -> T {
    return wrap(value)
}
"#;
    let helper = parse_module(helper_source);
    let dependency = parse_module(dependency_source);
    let root = parse_module(
        r#"
use dep.wrapped as wrapped

fn run() -> Bool {
    return wrapped(true)
}
"#,
    );
    let helper_import_path = vec!["helper".to_owned()];
    let helper_module = SpecializationModule {
        module_import_path: &helper_import_path,
        contents: helper_source,
        module: &helper,
    };

    let rendered = render_public_function_specialization_status_with_context(
        &["dep".to_owned()],
        function(&dependency, "wrapped"),
        dependency_source,
        &root,
        &dependency,
        &[helper_module],
        &mut BTreeSet::new(),
    );
    let PublicFunctionSpecializationRender::Rendered(rendered) = rendered else {
        panic!("wrapped should render grouped imported helper specialization");
    };

    assert!(
        rendered
            .declarations
            .contains("fn __ql_bridge_local_helper_wrap_value__generic_Bool(value: Bool) -> Bool")
    );
    assert!(
        rendered
            .declarations
            .contains("return __ql_bridge_local_helper_wrap_value__generic_Bool(value)")
    );
    assert!(!rendered.declarations.contains("return wrap(value)"));
    assert_eq!(rendered.call_rewrites.len(), 1);
    assert_eq!(
        rendered.call_rewrites[0].replacement,
        "__ql_bridge_local_dep_wrapped__generic_Bool"
    );
}
