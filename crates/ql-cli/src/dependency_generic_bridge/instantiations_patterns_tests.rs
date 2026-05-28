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
fn scans_nested_control_flow_blocks() {
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
    var total = 0
    while total < 1 {
        let current: Int = identity(1)
        total = current
    }
    loop {
        let flag: Bool = identity(true)
        break
    }
    for value in ["ready"] {
        let text: String = identity(value)
    }
    return total
}
"#,
    );

    let instantiations = collect_public_function_instantiations(
        &root,
        &["dep".to_owned()],
        &dependency,
        function(&dependency, "identity"),
    );

    assert!(
        instantiations
            .iter()
            .any(|item| item.get("T").map(String::as_str) == Some("Int"))
    );
    assert!(
        instantiations
            .iter()
            .any(|item| item.get("T").map(String::as_str) == Some("Bool"))
    );
    assert!(
        instantiations
            .iter()
            .any(|item| item.get("T").map(String::as_str) == Some("String"))
    );
}

#[test]
fn infers_substitutions_from_for_loop_pattern_bindings() {
    let dependency = parse_module(
        r#"
package dep

pub fn tag[T](value: T) -> Int {
    return 1
}
"#,
    );
    let root = parse_module(
        r#"
use dep.tag as tag

fn run() -> Int {
    var total = 0
    for value in [1, 2] {
        total = total + tag(value)
    }
    for (number, flag) in [(3, true), (4, false)] {
        total = total + tag(flag)
    }
    for [left, right] in [[5, 6], [7, 8]] {
        total = total + tag(right)
    }
    return total
}
"#,
    );

    let substitutions = collect_public_function_instantiations(
        &root,
        &["dep".to_owned()],
        &dependency,
        function(&dependency, "tag"),
    );

    assert_eq!(substitutions.len(), 2);
    assert!(
        substitutions
            .iter()
            .any(|item| item.get("T").map(String::as_str) == Some("Int"))
    );
    assert!(
        substitutions
            .iter()
            .any(|item| item.get("T").map(String::as_str) == Some("Bool"))
    );
}

#[test]
fn infers_substitutions_from_match_arm_pattern_bindings() {
    let dependency = parse_module(
        r#"
package dep

pub fn tag[T](value: T) -> Int {
    return 1
}
"#,
    );
    let root = parse_module(
        r#"
use dep.tag as tag

fn run() -> Int {
    let pair = (1, true)
    let values = [2, 3]
    let from_tuple = match pair {
        (number, flag) if flag => tag(flag),
        _ => 0,
    }
    let from_array = match values {
        [left, right] => tag(right),
        _ => 0,
    }
    return from_tuple + from_array
}
"#,
    );

    let substitutions = collect_public_function_instantiations(
        &root,
        &["dep".to_owned()],
        &dependency,
        function(&dependency, "tag"),
    );

    assert_eq!(substitutions.len(), 2);
    assert!(
        substitutions
            .iter()
            .any(|item| item.get("T").map(String::as_str) == Some("Int"))
    );
    assert!(
        substitutions
            .iter()
            .any(|item| item.get("T").map(String::as_str) == Some("Bool"))
    );
}

#[test]
fn infers_substitutions_from_generic_carrier_match_patterns() {
    let dependency = parse_module(
        r#"
package dep

pub fn tag[T](value: T) -> Int {
    return 1
}
"#,
    );
    let root = parse_module(
        r#"
use dep.tag as tag

enum Option[T] {
    Some(T),
    None,
}

enum Result[T, E] {
    Ok(T),
    Err(E),
}

fn run() -> Int {
    let maybe: Option[Int] = Option.Some(1)
    let result: Result[Bool, String] = Result.Err("bad")
    let from_option = match maybe {
        Option.Some(inner) => tag(inner),
        Option.None => 0,
    }
    let from_ok = match result {
        Result.Ok(flag) => tag(flag),
        Result.Err(_) => 0,
    }
    let from_err = match result {
        Result.Ok(_) => 0,
        Result.Err(error) => tag(error),
    }
    return from_option + from_ok + from_err
}
"#,
    );

    let substitutions = collect_public_function_instantiations(
        &root,
        &["dep".to_owned()],
        &dependency,
        function(&dependency, "tag"),
    );

    assert_eq!(substitutions.len(), 3);
    assert!(
        substitutions
            .iter()
            .any(|item| item.get("T").map(String::as_str) == Some("Int"))
    );
    assert!(
        substitutions
            .iter()
            .any(|item| item.get("T").map(String::as_str) == Some("Bool"))
    );
    assert!(
        substitutions
            .iter()
            .any(|item| item.get("T").map(String::as_str) == Some("String"))
    );
}

#[test]
fn infers_substitutions_from_custom_generic_enum_match_patterns() {
    let dependency = parse_module(
        r#"
package dep

pub fn tag[T](value: T) -> Int {
    return 1
}
"#,
    );
    let root = parse_module(
        r#"
use dep.tag as tag

enum Boxed[T] {
    Item(T),
    Empty,
}

fn run() -> Int {
    let value = Boxed.Item(7)
    return match value {
        Boxed.Item(inner) => tag(inner),
        Boxed.Empty => 0,
    }
}
"#,
    );

    let substitutions = collect_public_function_instantiations(
        &root,
        &["dep".to_owned()],
        &dependency,
        function(&dependency, "tag"),
    );

    assert_eq!(substitutions.len(), 1);
    assert_eq!(
        substitutions
            .iter()
            .next()
            .and_then(|item| item.get("T"))
            .map(String::as_str),
        Some("Int")
    );
}

#[test]
fn infers_substitutions_from_untyped_tuple_destructuring_bindings() {
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
    let (number, flag) = (1, true)
    let picked: Int = identity(number)
    let matched: Bool = identity(flag)
    if matched {
        return picked
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
fn infers_substitutions_from_array_destructuring_bindings() {
    let dependency = parse_module(
        r#"
package dep

pub fn tag[T](value: T) -> Int {
    return 1
}
"#,
    );
    let root = parse_module(
        r#"
use dep.tag as tag

fn run() -> Int {
    let [first, second, third] = [1, 2, 3]
    let from_inferred = tag(second)
    let [enabled, disabled]: [Bool; 2] = [true, false]
    let from_explicit = tag(enabled)
    return from_inferred + from_explicit
}
"#,
    );

    let substitutions = collect_public_function_instantiations(
        &root,
        &["dep".to_owned()],
        &dependency,
        function(&dependency, "tag"),
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
