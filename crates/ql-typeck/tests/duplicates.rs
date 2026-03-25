mod support;

use support::diagnostic_messages;

#[test]
fn detects_duplicate_top_level_definitions() {
    let diagnostics = diagnostic_messages(
        r#"
struct User {}
fn User() {}
"#,
    );

    assert!(diagnostics.contains(&"duplicate top-level definition `User`".to_string()));
}

#[test]
fn detects_duplicate_generic_parameters() {
    let diagnostics = diagnostic_messages(
        r#"
fn id[T, T](value: T) -> T {
    value
}
"#,
    );

    assert!(diagnostics.contains(&"duplicate generic parameter `T`".to_string()));
}

#[test]
fn detects_duplicate_function_parameters() {
    let diagnostics = diagnostic_messages(
        r#"
fn add(left: Int, left: Int) -> Int {
    left
}
"#,
    );

    assert!(diagnostics.contains(&"duplicate parameter `left`".to_string()));
}

#[test]
fn detects_duplicate_closure_parameters() {
    let diagnostics = diagnostic_messages(
        r#"
fn main() {
    let closure = (item, item) => item;
}
"#,
    );

    assert!(diagnostics.contains(&"duplicate closure parameter `item`".to_string()));
}

#[test]
fn detects_duplicate_pattern_bindings() {
    let diagnostics = diagnostic_messages(
        r#"
fn main() {
    let (left, left) = pair;
}
"#,
    );

    assert!(diagnostics.contains(&"duplicate binding in pattern `left`".to_string()));
}

#[test]
fn detects_duplicate_struct_pattern_fields() {
    let diagnostics = diagnostic_messages(
        r#"
fn main() {
    let Point { x: left, x: right } = point;
}
"#,
    );

    assert!(diagnostics.contains(&"duplicate field in struct pattern `x`".to_string()));
}

#[test]
fn detects_duplicate_shorthand_bindings_in_struct_patterns() {
    let diagnostics = diagnostic_messages(
        r#"
fn main() {
    let Point { x, y: x } = point;
}
"#,
    );

    assert!(diagnostics.contains(&"duplicate binding in pattern `x`".to_string()));
}

#[test]
fn detects_duplicate_struct_fields() {
    let diagnostics = diagnostic_messages(
        r#"
struct Point {
    x: Int,
    x: Int,
}
"#,
    );

    assert!(diagnostics.contains(&"duplicate field in struct `x`".to_string()));
}

#[test]
fn detects_duplicate_enum_variants() {
    let diagnostics = diagnostic_messages(
        r#"
enum Result {
    Ok,
    Ok,
}
"#,
    );

    assert!(diagnostics.contains(&"duplicate enum variant `Ok`".to_string()));
}

#[test]
fn detects_duplicate_enum_variant_struct_fields() {
    let diagnostics = diagnostic_messages(
        r#"
enum Result {
    Config { enabled: Bool, enabled: Bool },
}
"#,
    );

    assert!(diagnostics.contains(&"duplicate field in enum variant `enabled`".to_string()));
}

#[test]
fn detects_duplicate_struct_literal_fields() {
    let diagnostics = diagnostic_messages(
        r#"
fn main() {
    Point { x: 1, x: 2 };
}
"#,
    );

    assert!(diagnostics.contains(&"duplicate field in struct literal `x`".to_string()));
}

#[test]
fn detects_duplicate_named_call_arguments() {
    let diagnostics = diagnostic_messages(
        r#"
fn main() {
    run(left: 1, left: 2);
}
"#,
    );

    assert!(diagnostics.contains(&"duplicate named call argument `left`".to_string()));
}

#[test]
fn detects_positional_arguments_after_named_arguments() {
    let diagnostics = diagnostic_messages(
        r#"
fn run(left: Int, right: Int) -> Int {
    return left + right
}

fn main() -> Int {
    return run(left: 1, 2)
}
"#,
    );

    assert!(
        diagnostics
            .contains(&"positional argument cannot appear after named arguments".to_string())
    );
}

#[test]
fn detects_duplicate_methods_in_trait_impl_and_extend_blocks() {
    let diagnostics = diagnostic_messages(
        r#"
trait Api {
    fn open() -> Int
    fn open() -> Int
}

impl Service {
    fn open() -> Int { 1 }
    fn open() -> Int { 2 }
}

extend Service {
    fn ping() -> Int { 1 }
    fn ping() -> Int { 2 }
}
"#,
    );

    assert!(diagnostics.contains(&"duplicate method in trait `open`".to_string()));
    assert!(diagnostics.contains(&"duplicate method in impl `open`".to_string()));
    assert!(diagnostics.contains(&"duplicate method in extend block `ping`".to_string()));
}
