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
fn infers_substitutions_from_generic_array_parameters() {
    let dependency = parse_module(
        r#"
package dep

pub fn fixed_first[T](values: [T; 3]) -> T {
    return values[0]
}
"#,
    );
    let root = parse_module(
        r#"
use dep.fixed_first as fixed_first

fn run() -> Int {
    let value: Int = fixed_first([1, 2 + 3, 4])
    let flag: Bool = fixed_first([true, false, true])
    if flag {
        return value
    }
    return 0
}
"#,
    );

    let substitutions = collect_public_function_instantiations(
        &root,
        &["dep".to_owned()],
        &dependency,
        function(&dependency, "fixed_first"),
    );

    assert_eq!(substitutions.len(), 2);
    assert!(
        substitutions
            .iter()
            .any(|item| { item.get("T").map(String::as_str) == Some("Int") })
    );
    assert!(
        substitutions
            .iter()
            .any(|item| { item.get("T").map(String::as_str) == Some("Bool") })
    );
}

#[test]
fn infers_substitutions_from_generic_array_length_parameters() {
    let dependency = parse_module(
        r#"
package dep

pub fn first[T, N](values: [T; N]) -> T {
    return values[0]
}
"#,
    );
    let root = parse_module(
        r#"
use dep.first as first

fn run() -> Int {
    let value: Int = first([1, 2 + 3, 4])
    let flag: Bool = first([true, false, true, false])
    if flag {
        return value
    }
    return 0
}
"#,
    );

    let substitutions = collect_public_function_instantiations(
        &root,
        &["dep".to_owned()],
        &dependency,
        function(&dependency, "first"),
    );

    assert_eq!(substitutions.len(), 2);
    assert!(substitutions.iter().any(|item| {
        item.get("T").map(String::as_str) == Some("Int")
            && item.get("N").map(String::as_str) == Some("3")
    }));
    assert!(substitutions.iter().any(|item| {
        item.get("T").map(String::as_str) == Some("Bool")
            && item.get("N").map(String::as_str) == Some("4")
    }));
}

#[test]
fn infers_substitutions_from_repeat_array_literals() {
    let dependency = parse_module(
        r#"
package dep

pub fn mirror[T, N](values: [T; N]) -> [T; N] {
    return values
}
"#,
    );
    let root = parse_module(
        r#"
use dep.mirror as mirror

fn run() -> Int {
    let values: [Int; 3] = mirror([7; 3])
    return values[0]
}
"#,
    );

    let substitutions = collect_public_function_instantiations(
        &root,
        &["dep".to_owned()],
        &dependency,
        function(&dependency, "mirror"),
    );

    assert_eq!(substitutions.len(), 1);
    let item = substitutions
        .iter()
        .next()
        .expect("one repeat-array substitution should be inferred");
    assert_eq!(item.get("T").map(String::as_str), Some("Int"));
    assert_eq!(item.get("N").map(String::as_str), Some("3"));
}

#[test]
fn infers_array_substitution_from_partially_typed_array_literal_items() {
    let dependency = parse_module(
        r#"
package dep

pub fn reverse[T, N](values: [T; N]) -> [T; N] {
    return values
}
"#,
    );
    let root = parse_module(
        r#"
use dep.reverse as reverse

fn hidden_value() -> Int {
    return 2
}

fn run() -> Int {
    let values: [Int; 3] = reverse([1, hidden_value(), 3])
    return values[0]
}
"#,
    );

    let substitutions = collect_public_function_instantiations(
        &root,
        &["dep".to_owned()],
        &dependency,
        function(&dependency, "reverse"),
    );

    assert_eq!(substitutions.len(), 1);
    let substitutions = substitutions
        .iter()
        .next()
        .expect("one array literal substitution should be inferred");
    assert_eq!(substitutions.get("T").map(String::as_str), Some("Int"));
    assert_eq!(substitutions.get("N").map(String::as_str), Some("3"));
}
