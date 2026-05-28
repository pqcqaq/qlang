use super::super::enum_bindings::collect_root_call_enum_type_bindings;
use super::super::struct_bindings::collect_root_call_struct_type_bindings;
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
fn infers_substitutions_from_scalar_expressions() {
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

fn run() -> Int {
    let number: Int = identity(1 + 2)
    let flag: Bool = identity(!(false || false))
    let ordered: Bool = identity(1 < 2)
    if flag && ordered {
        return number
    }
    return 0
}
"#,
    );

    let substitutions = collect_public_function_instantiations(
        &root,
        &["dep".to_owned()],
        &dependency,
        function(&dependency, "identity"),
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
fn infers_substitutions_from_tuple_and_array_literals() {
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

fn run() -> Int {
    let pair: (Int, Bool) = identity((1 + 2, !false))
    let values: [Int; 3] = identity([1, 2 + 3, 4])
    if pair[1] {
        return values[0]
    }
    return 0
}
"#,
    );

    let substitutions = collect_public_function_instantiations(
        &root,
        &["dep".to_owned()],
        &dependency,
        function(&dependency, "identity"),
    );

    assert_eq!(substitutions.len(), 2);
    assert!(
        substitutions
            .iter()
            .any(|item| { item.get("T").map(String::as_str) == Some("(Int, Bool)") })
    );
    assert!(
        substitutions
            .iter()
            .any(|item| { item.get("T").map(String::as_str) == Some("[Int; 3]") })
    );
}

#[test]
fn infers_substitutions_from_projection_and_control_flow_expressions() {
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

fn run(flag: Bool) -> Int {
    let values: [Int; 3] = [1, 2, 3]
    let pair: (Int, Bool) = (5, flag)
    let picked: Int = identity(values[1])
    let tuple_flag: Bool = identity(pair[1])
    let selected: Int = identity(if tuple_flag { picked } else { 0 })
    let matched: Bool = identity(match selected {
        0 => false,
        _ => tuple_flag,
    })
    if matched {
        return selected
    }
    return 0
}
"#,
    );

    let substitutions = collect_public_function_instantiations(
        &root,
        &["dep".to_owned()],
        &dependency,
        function(&dependency, "identity"),
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
fn scans_calls_nested_in_expression_traversal_forms() {
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

struct Pair {
    number: Int,
    flag: Bool,
}

fn run(values: [Int; 3], flag: Bool) -> Int {
    let pair = Pair {
        number: identity(1) + values[identity(0)],
        flag: match flag {
            _ if identity(true) => identity(true),
            _ => false,
        },
    }
    if pair.flag {
        return pair.number
    }
    return 0
}
"#,
    );

    let instantiations = collect_public_function_call_instantiations(
        &root,
        &["dep".to_owned()],
        function(&dependency, "identity"),
        &FunctionTypeBindings::new(),
        &collect_root_call_enum_type_bindings(&root, &["dep".to_owned()], &dependency),
        &collect_root_call_struct_type_bindings(&root, &["dep".to_owned()], &dependency),
    );

    assert_eq!(instantiations.len(), 4);
    assert_eq!(
        instantiations
            .iter()
            .filter(|item| item.substitutions.get("T").map(String::as_str) == Some("Int"))
            .count(),
        2
    );
    assert_eq!(
        instantiations
            .iter()
            .filter(|item| item.substitutions.get("T").map(String::as_str) == Some("Bool"))
            .count(),
        2
    );
}
