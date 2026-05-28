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
    );

    assert_eq!(instantiations.len(), 1);
    assert_eq!(
        instantiations[0].substitutions.get("T").map(String::as_str),
        Some("Int")
    );
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
        function(&dependency, "ok"),
    );
    let err_instantiations = collect_public_function_instantiations(
        &root,
        &["std".to_owned(), "result".to_owned()],
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
