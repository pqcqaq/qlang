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
fn reports_tuple_pattern_type_mismatches() {
    let diagnostics = diagnostic_messages(
        r#"
fn main() -> Int {
    let (left, right) = 1
    return 0
}
"#,
    );

    assert!(diagnostics.contains(&"tuple pattern requires a tuple value, found `Int`".to_string()));
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
fn reports_call_argument_type_mismatches_for_extern_block_functions() {
    let diagnostics = diagnostic_messages(
        r#"
extern "c" {
    fn q_add(left: Int, right: Int) -> Int
}

fn main() -> Int {
    return q_add(true, 2)
}
"#,
    );

    assert!(
        diagnostics
            .contains(&"call argument has type mismatch: expected `Int`, found `Bool`".to_string())
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
fn reports_unknown_struct_members() {
    let diagnostics = diagnostic_messages(
        r#"
struct User {
    name: String,
}

fn main() -> Int {
    let user = User { name: "ql" }
    user.age
    return 0
}
"#,
    );

    assert!(diagnostics.contains(&"unknown member `age` on type `User`".to_string()));
}

#[test]
fn accepts_method_selection_without_field_false_positives() {
    let diagnostics = diagnostic_messages(
        r#"
struct Counter {
    value: Int,
}

impl Counter {
    fn get(self) -> Int {
        return self.value
    }

    fn next(self) -> Int {
        return self.get()
    }
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected no diagnostics, got {diagnostics:?}"
    );
}

#[test]
fn reports_method_call_argument_type_mismatches_for_unique_member_selection() {
    let diagnostics = diagnostic_messages(
        r#"
struct Counter {
    value: Int,
}

impl Counter {
    fn add(self, delta: Int) -> Int {
        return self.value + delta
    }
}

fn main() -> Int {
    let counter = Counter { value: 1 }
    return counter.add(true)
}
"#,
    );

    assert!(
        diagnostics
            .contains(&"call argument has type mismatch: expected `Int`, found `Bool`".to_string())
    );
}

#[test]
fn prefers_impl_methods_over_extend_candidates() {
    let diagnostics = diagnostic_messages(
        r#"
struct Counter {
    value: Int,
}

impl Counter {
    fn read(self, delta: Int) -> Int {
        return self.value + delta
    }
}

extend Counter {
    fn read(self) -> Int {
        return self.value
    }
}

fn main() -> Int {
    let counter = Counter { value: 1 }
    return counter.read(true)
}
"#,
    );

    assert!(
        diagnostics
            .contains(&"call argument has type mismatch: expected `Int`, found `Bool`".to_string()),
        "expected impl method to win over extend candidate, got {diagnostics:?}"
    );
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

#[test]
fn reports_pattern_root_type_mismatches() {
    let diagnostics = diagnostic_messages(
        r#"
struct User {
    name: String,
}

struct Point {
    x: Int,
}

fn main() -> Int {
    let Point { x } = User { name: "ql" }
    return x
}
"#,
    );

    assert!(
        diagnostics.contains(
            &"struct pattern has type mismatch: expected `User`, found `Point`".to_string()
        )
    );
}

#[test]
fn reports_variant_pattern_type_mismatches() {
    let diagnostics = diagnostic_messages(
        r#"
struct User {
    name: String,
}

enum Result {
    Ok(Int),
    Err(String),
}

fn main() -> Int {
    let user = User { name: "ql" }
    match user {
        Result.Ok(value) => value,
        _ => 0,
    }
}
"#,
    );

    assert!(diagnostics.contains(
        &"tuple-struct pattern has type mismatch: expected `User`, found `Result`".to_string()
    ));
}

#[test]
fn reports_equality_operand_mismatches() {
    let diagnostics = diagnostic_messages(
        r#"
fn main() -> Bool {
    return 1 == "x"
}
"#,
    );

    assert!(diagnostics.contains(
        &"equality operator `==` expects compatible operands, found `Int` and `String`".to_string()
    ));
}
