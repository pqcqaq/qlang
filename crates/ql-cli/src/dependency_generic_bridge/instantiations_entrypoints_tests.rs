use super::*;

fn parse_module(source: &str) -> Module {
    ql_parser::parse_source(source).expect("test source should parse")
}

fn function<'a>(module: &'a Module, name: &str) -> &'a FunctionDecl {
    module
        .items
        .iter()
        .find_map(|item| match &item.kind {
            ItemKind::Function(function) if function.name == name => Some(function),
            _ => None,
        })
        .expect("test function should exist")
}

#[test]
fn local_instantiations_skip_generic_root_function_bodies() {
    let module = parse_module(
        r#"
fn identity[T](value: T) -> T {
    return value
}

fn wrapper[T](value: T) -> T {
    return identity(value)
}

fn run() -> Int {
    return identity(1)
}
"#,
    );

    let instantiations = collect_local_function_call_instantiations(
        &module,
        function(&module, "identity"),
        &FunctionTypeBindings::new(),
        &collect_root_call_enum_type_bindings(&module, &[], &module),
        &collect_root_call_struct_type_bindings(&module, &[], &module),
    );

    assert_eq!(instantiations.len(), 1);
    assert_eq!(
        instantiations[0].substitutions.get("T").map(String::as_str),
        Some("Int")
    );
}

#[test]
fn infers_substitution_from_later_argument_when_nested_call_arg_is_untyped() {
    let dependency = parse_module(
        r#"
package std.option

pub enum Option[T] {
    Some(T),
    None,
}

pub fn unwrap_or[T](value: Option[T], fallback: T) -> T {
    return fallback
}
"#,
    );
    let root = parse_module(
        r#"
use std.option.some as option_some
use std.option.unwrap_or as option_unwrap_or

fn run() -> Int {
    return option_unwrap_or(option_some(42), 0)
}
"#,
    );

    let instantiations = collect_public_function_instantiations(
        &root,
        &["std".to_owned(), "option".to_owned()],
        &dependency,
        function(&dependency, "unwrap_or"),
    );

    assert_eq!(instantiations.len(), 1);
    let substitutions = instantiations
        .iter()
        .next()
        .expect("one substitution should be inferred");
    assert_eq!(substitutions.get("T").map(String::as_str), Some("Int"));
}

#[test]
fn infers_substitution_from_named_arguments() {
    let dependency = parse_module(
        r#"
package dep

pub fn choose[T](fallback: T, value: T) -> T {
    return value
}
"#,
    );
    let root = parse_module(
        r#"
use dep.choose as choose

fn run() -> Int {
    return choose(value: 42, fallback: 0)
}
"#,
    );

    let instantiations = collect_public_function_instantiations(
        &root,
        &["dep".to_owned()],
        &dependency,
        function(&dependency, "choose"),
    );

    assert_eq!(instantiations.len(), 1);
    let substitutions = instantiations
        .iter()
        .next()
        .expect("one named-argument substitution should be inferred");
    assert_eq!(substitutions.get("T").map(String::as_str), Some("Int"));
}

#[test]
fn scans_global_and_struct_default_instantiations() {
    let dependency = parse_module(
        r#"
package dep

pub fn identity[T](value: T) -> T {
    return value
}
"#,
    );
    let root = parse_module(
        r#"
use dep.identity as identity

const DEFAULT_COUNT: Int = identity(1)

struct Settings {
    enabled: Bool = identity(true),
}

fn run() -> Int {
    return DEFAULT_COUNT
}
"#,
    );

    let instantiations = collect_public_function_instantiations(
        &root,
        &["dep".to_owned()],
        &dependency,
        function(&dependency, "identity"),
    );

    assert_eq!(instantiations.len(), 2);
    assert!(
        instantiations
            .iter()
            .any(|item| { item.get("T").map(String::as_str) == Some("Int") })
    );
    assert!(
        instantiations
            .iter()
            .any(|item| { item.get("T").map(String::as_str) == Some("Bool") })
    );
}
