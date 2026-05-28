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
fn infers_substitution_from_single_field_generic_variant_call() {
    let dependency = parse_module(
        r#"
package std.option

pub enum Option[T] {
    Some(T),
    None,
}

pub fn is_some[T](value: Option[T]) -> Bool {
    return match value {
        Option.Some(_) => true,
        Option.None => false,
    }
}
"#,
    );
    let root = parse_module(
        r#"
use std.option.Option as Option
use std.option.is_some as option_is_some

fn run() -> Int {
    if option_is_some(Option.Some(42)) {
        return 0
    }
    return 1
}
"#,
    );

    let instantiations = collect_public_function_instantiations(
        &root,
        &["std".to_owned(), "option".to_owned()],
        &dependency,
        function(&dependency, "is_some"),
    );

    assert_eq!(instantiations.len(), 1);
    let substitutions = instantiations
        .iter()
        .next()
        .expect("one substitution should be inferred");
    assert_eq!(substitutions.get("T").map(String::as_str), Some("Int"));
}

#[test]
fn infers_generic_enum_argument_from_typed_local_binding() {
    let dependency = parse_module(
        r#"
package std.option

pub enum Option[T] {
    Some(T),
    None,
}

pub fn is_none[T](value: Option[T]) -> Bool {
    return match value {
        Option.Some(_) => false,
        Option.None => true,
    }
}
"#,
    );
    let root = parse_module(
        r#"
use std.option.Option as Option
use std.option.is_none as option_is_none

fn run() -> Int {
    let value: Option[Int] = Option.None
    if option_is_none(value) {
        return 0
    }
    return 1
}
"#,
    );

    let instantiations = collect_public_function_instantiations(
        &root,
        &["std".to_owned(), "option".to_owned()],
        &dependency,
        function(&dependency, "is_none"),
    );

    assert_eq!(instantiations.len(), 1);
    let substitutions = instantiations
        .iter()
        .next()
        .expect("one typed option substitution should be inferred");
    assert_eq!(substitutions.get("T").map(String::as_str), Some("Int"));
}

#[test]
fn infers_generic_enum_argument_when_target_reexports_imported_carrier() {
    let dependency = parse_module(
        r#"
package std.test

use std.option.Option as Option

pub fn expect_option_none[T](value: Option[T]) -> Int {
    return match value {
        Option.Some(_) => 1,
        Option.None => 0,
    }
}
"#,
    );
    let root = parse_module(
        r#"
use std.option.Option as Option
use std.test.expect_option_none as expect_option_none

fn run() -> Int {
    let value: Option[String] = Option.None
    return expect_option_none(value)
}
"#,
    );

    let instantiations = collect_public_function_instantiations(
        &root,
        &["std".to_owned(), "test".to_owned()],
        &dependency,
        function(&dependency, "expect_option_none"),
    );

    assert_eq!(instantiations.len(), 1);
    let substitutions = instantiations
        .iter()
        .next()
        .expect("one reexported option substitution should be inferred");
    assert_eq!(substitutions.get("T").map(String::as_str), Some("String"));
}

#[test]
fn infers_generic_enum_argument_inside_nested_status_aggregates() {
    let dependency = parse_module(
        r#"
package std.test

use std.option.Option as Option

pub fn expect_option_none[T](value: Option[T]) -> Int {
    return match value {
        Option.Some(_) => 1,
        Option.None => 0,
    }
}
"#,
    );
    let root = parse_module(
        r#"
use std.option.Option as Option
use std.test.expect_option_none as expect_option_none

fn check_int(actual: Int, expected: Int) -> Int {
    if actual == expected {
        return 0
    }
    return 1
}

fn sum_statuses[N](statuses: [Int; N]) -> Int {
    var total = 0
    for status in statuses {
        total = total + status
    }
    return total
}

fn run() -> Int {
    let missing: Option[String] = Option.None
    return sum_statuses([check_int(expect_option_none(missing), 0)])
}
"#,
    );

    let instantiations = collect_public_function_instantiations(
        &root,
        &["std".to_owned(), "test".to_owned()],
        &dependency,
        function(&dependency, "expect_option_none"),
    );

    assert_eq!(instantiations.len(), 1);
    let substitutions = instantiations
        .iter()
        .next()
        .expect("one nested option substitution should be inferred");
    assert_eq!(substitutions.get("T").map(String::as_str), Some("String"));
}

#[test]
fn infers_substitution_from_explicit_result_context() {
    let dependency = parse_module(
        r#"
package std.result

pub enum Result[T, E] {
    Ok(T),
    Err(E),
}

pub fn ok[T, E](value: T) -> Result[T, E] {
    return Result.Ok(value)
}

pub fn err[T, E](error: E) -> Result[T, E] {
    return Result.Err(error)
}
"#,
    );
    let root = parse_module(
        r#"
use std.result.Result as Result
use std.result.ok as result_ok
use std.result.err as result_err

fn make_ok() -> Result[Int, Int] {
    return result_ok(42)
}

fn run() -> Int {
    let failed: Result[Int, Int] = result_err(3)
    return 0
}
"#,
    );

    let ok_instantiations = collect_public_function_instantiations(
        &root,
        &["std".to_owned(), "result".to_owned()],
        &dependency,
        function(&dependency, "ok"),
    );
    let err_instantiations = collect_public_function_instantiations(
        &root,
        &["std".to_owned(), "result".to_owned()],
        &dependency,
        function(&dependency, "err"),
    );

    assert_eq!(ok_instantiations.len(), 1);
    let ok = ok_instantiations
        .iter()
        .next()
        .expect("ok should infer one substitution");
    assert_eq!(ok.get("T").map(String::as_str), Some("Int"));
    assert_eq!(ok.get("E").map(String::as_str), Some("Int"));

    assert_eq!(err_instantiations.len(), 1);
    let err = err_instantiations
        .iter()
        .next()
        .expect("err should infer one substitution");
    assert_eq!(err.get("T").map(String::as_str), Some("Int"));
    assert_eq!(err.get("E").map(String::as_str), Some("Int"));
}

#[test]
fn infers_nested_call_substitution_from_outer_parameter_context() {
    let dependency = parse_module(
        r#"
package std.result

pub enum Option[T] {
    Some(T),
    None,
}

pub enum Result[T, E] {
    Ok(T),
    Err(E),
}

pub fn to_option[T, E](value: Result[T, E]) -> Option[T] {
    return match value {
        Result.Ok(inner) => Option.Some(inner),
        Result.Err(_) => Option.None,
    }
}

pub fn value_or[T](value: Option[T], fallback: T) -> T {
    return match value {
        Option.Some(inner) => inner,
        Option.None => fallback,
    }
}

pub fn identity[T](value: T) -> T {
    return value
}
"#,
    );
    let root = parse_module(
        r#"
use std.result.Option as Option
use std.result.Result as Result
use std.result.identity as identity
use std.result.to_option as result_to_option
use std.result.value_or as option_value_or

fn run(value: Result[Int, Int]) -> Int {
    return option_value_or(result_to_option(value), 0)
}

fn nested(value: Result[Int, Int]) -> Option[Int] {
    return identity(result_to_option(value))
}
"#,
    );

    let instantiations = collect_public_function_instantiations(
        &root,
        &["std".to_owned(), "result".to_owned()],
        &dependency,
        function(&dependency, "to_option"),
    );

    assert_eq!(instantiations.len(), 1);
    let substitutions = instantiations
        .iter()
        .next()
        .expect("to_option should infer one nested substitution");
    assert_eq!(substitutions.get("T").map(String::as_str), Some("Int"));
    assert_eq!(substitutions.get("E").map(String::as_str), Some("Int"));
}

#[test]
fn infers_zero_argument_substitution_from_explicit_option_context() {
    let dependency = parse_module(
        r#"
package std.option

pub enum Option[T] {
    Some(T),
    None,
}

pub fn none_option[T]() -> Option[T] {
    return Option.None
}
"#,
    );
    let root = parse_module(
        r#"
use std.option.Option as Option
use std.option.none_option as option_none

fn make_none() -> Option[Int] {
    return option_none()
}

fn run() -> Int {
    let value: Option[Int] = option_none()
    return 0
}
"#,
    );

    let instantiations = collect_public_function_instantiations(
        &root,
        &["std".to_owned(), "option".to_owned()],
        &dependency,
        function(&dependency, "none_option"),
    );

    assert_eq!(instantiations.len(), 1);
    let substitutions = instantiations
        .iter()
        .next()
        .expect("none_option should infer one substitution");
    assert_eq!(substitutions.get("T").map(String::as_str), Some("Int"));
}

#[test]
fn infers_zero_argument_substitutions_from_composite_expected_contexts() {
    let dependency = parse_module(
        r#"
package std.option

pub enum Option[T] {
    Some(T),
    None,
}

pub fn none_option[T]() -> Option[T] {
    return Option.None
}
"#,
    );
    let root = parse_module(
        r#"
use std.option.Option as Option
use std.option.none_option as option_none

fn run() -> Int {
    let array_values: [Option[Int]; 2] = [option_none(), Option.Some(1)]
    let repeated_values: [Option[String]; 3] = [option_none(); 3]
    let tuple_values: (Option[Bool], Int) = (option_none(), 1)
    return 0
}
"#,
    );

    let instantiations = collect_public_function_instantiations(
        &root,
        &["std".to_owned(), "option".to_owned()],
        &dependency,
        function(&dependency, "none_option"),
    );

    assert_eq!(instantiations.len(), 3);
    assert!(
        instantiations
            .iter()
            .any(|item| item.get("T").map(String::as_str) == Some("Int"))
    );
    assert!(
        instantiations
            .iter()
            .any(|item| item.get("T").map(String::as_str) == Some("String"))
    );
    assert!(
        instantiations
            .iter()
            .any(|item| item.get("T").map(String::as_str) == Some("Bool"))
    );
}

#[test]
fn infers_zero_argument_substitutions_from_struct_field_expected_contexts() {
    let dependency = parse_module(
        r#"
package std.option

pub enum Option[T] {
    Some(T),
    None,
}

pub fn none_option[T]() -> Option[T] {
    return Option.None
}
"#,
    );
    let root = parse_module(
        r#"
use std.option.Option as Option
use std.option.none_option as option_none

struct OptionBox[T] {
    value: Option[T],
}

struct OptionPair[T] {
    left: Option[T],
    right: T,
}

fn run() -> Int {
    let int_box: OptionBox[Int] = OptionBox { value: option_none() }
    let string_pair: OptionPair[String] = OptionPair { left: option_none(), right: "ready" }
    return 0
}
"#,
    );

    let instantiations = collect_public_function_instantiations(
        &root,
        &["std".to_owned(), "option".to_owned()],
        &dependency,
        function(&dependency, "none_option"),
    );

    assert_eq!(instantiations.len(), 2);
    assert!(
        instantiations
            .iter()
            .any(|item| item.get("T").map(String::as_str) == Some("Int"))
    );
    assert!(
        instantiations
            .iter()
            .any(|item| item.get("T").map(String::as_str) == Some("String"))
    );
}
