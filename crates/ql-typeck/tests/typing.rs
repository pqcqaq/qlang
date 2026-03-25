mod support;

use support::diagnostic_messages;

#[test]
fn accepts_direct_closures_for_callable_parameters() {
    let diagnostics = diagnostic_messages(
        r#"
fn apply(f: (Int) -> Int, value: Int) -> Int {
    return f(value)
}

fn main() -> Int {
    return apply((x) => x + 1, 2)
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected no diagnostics, got {diagnostics:?}"
    );
}

#[test]
fn accepts_tuple_multi_return_destructuring() {
    let diagnostics = diagnostic_messages(
        r#"
fn div_rem(left: Int, right: Int) -> (Int, Int) {
    return (left / right, left % right)
}

fn main() -> Int {
    let (q, r) = div_rem(10, 3)
    return q + r
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected no diagnostics, got {diagnostics:?}"
    );
}

#[test]
fn accepts_struct_literals_that_use_default_fields() {
    let diagnostics = diagnostic_messages(
        r#"
struct User {
    name: String,
    age: Int = 0,
}

fn make(name: String) -> User {
    return User { name }
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected no diagnostics, got {diagnostics:?}"
    );
}

#[test]
fn reports_return_type_mismatches() {
    let diagnostics = diagnostic_messages(
        r#"
fn main() -> Int {
    return "oops"
}
"#,
    );

    assert!(
        diagnostics.contains(
            &"return value has type mismatch: expected `Int`, found `String`".to_string()
        )
    );
}

#[test]
fn reports_non_bool_conditions() {
    let diagnostics = diagnostic_messages(
        r#"
fn main() -> Int {
    while 1 {
        break
    }
    return 0
}
"#,
    );

    assert!(
        diagnostics.contains(&"while condition must have type `Bool`, found `Int`".to_string())
    );
}

#[test]
fn reports_tuple_pattern_arity_mismatches() {
    let diagnostics = diagnostic_messages(
        r#"
fn main() -> Int {
    let (left, right, extra) = (1, 2)
    return left + right
}
"#,
    );

    assert!(diagnostics.contains(&"tuple pattern expects 3 item(s), found 2".to_string()));
}

#[test]
fn reports_call_arity_mismatches() {
    let diagnostics = diagnostic_messages(
        r#"
fn add(left: Int, right: Int) -> Int {
    return left + right
}

fn main() -> Int {
    return add(1)
}
"#,
    );

    assert!(diagnostics.contains(&"expected 2 argument(s), found 1".to_string()));
}

#[test]
fn reports_call_argument_type_mismatches() {
    let diagnostics = diagnostic_messages(
        r#"
fn add(left: Int, right: Int) -> Int {
    return left + right
}

fn main() -> Int {
    return add(1, "x")
}
"#,
    );

    assert!(
        diagnostics.contains(
            &"call argument has type mismatch: expected `Int`, found `String`".to_string()
        )
    );
}

#[test]
fn reports_call_argument_type_mismatches_through_const_callable_values() {
    let diagnostics = diagnostic_messages(
        r#"
const APPLY: (Int) -> Int = (value) => value + 1

fn main() -> Int {
    return APPLY("x")
}
"#,
    );

    assert!(
        diagnostics.contains(
            &"call argument has type mismatch: expected `Int`, found `String`".to_string()
        )
    );
}

#[test]
fn reports_non_callable_values() {
    let diagnostics = diagnostic_messages(
        r#"
fn main() -> Int {
    let value = 1
    return value()
}
"#,
    );

    assert!(diagnostics.contains(&"cannot call value of type `Int`".to_string()));
}

#[test]
fn reports_struct_literal_shape_and_field_type_errors() {
    let diagnostics = diagnostic_messages(
        r#"
struct User {
    name: String,
    age: Int = 0,
}

fn main() -> User {
    return User { age: "old", missing: 1 }
}
"#,
    );

    assert!(diagnostics.contains(
        &"struct literal field has type mismatch: expected `Int`, found `String`".to_string()
    ));
    assert!(diagnostics.contains(&"unknown field `missing` in struct literal".to_string()));
    assert!(diagnostics.contains(&"missing required field `name` in struct literal".to_string()));
}
