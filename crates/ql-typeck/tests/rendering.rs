mod support;

use support::rendered_diagnostics;

#[test]
fn rendered_duplicate_diagnostics_anchor_to_duplicate_name_span() {
    let source = r#"
fn id[T, T](value: T, value: T) -> T {
    value
}
"#;
    let rendered = rendered_diagnostics(source);

    assert!(rendered.contains("error: sample.ql:2:10: duplicate generic parameter `T`"));
    assert!(rendered.contains("error: sample.ql:2:23: duplicate parameter `value`"));
}

#[test]
fn rendered_named_call_argument_duplicates_anchor_to_argument_label() {
    let source = r#"
fn main() {
    run(left: 1, left: 2);
}
"#;
    let rendered = rendered_diagnostics(source);

    assert!(rendered.contains("error: sample.ql:3:18: duplicate named call argument `left`"));
}

#[test]
fn rendered_positional_after_named_arguments_anchor_to_offending_argument() {
    let source = r#"
fn run(left: Int, right: Int) -> Int {
    return left + right
}

fn main() -> Int {
    return run(left: 1, 2)
}
"#;
    let rendered = rendered_diagnostics(source);

    assert!(rendered.contains(
        "error: sample.ql:7:25: positional argument cannot appear after named arguments"
    ));
}

#[test]
fn rendered_return_type_mismatches_anchor_to_return_expression() {
    let source = r#"
fn main() -> Int {
    return "oops"
}
"#;
    let rendered = rendered_diagnostics(source);

    assert!(rendered.contains(
        "error: sample.ql:3:5: return value has type mismatch: expected `Int`, found `String`"
    ));
}

#[test]
fn rendered_assignment_immutability_diagnostics_anchor_to_the_target() {
    let source = r#"
fn main() -> Int {
    let value = 1
    value = 2
    return value
}
"#;
    let rendered = rendered_diagnostics(source);

    assert!(rendered.contains(
        "error: sample.ql:4:5: cannot assign to immutable local `value`; declare it with `var`"
    ));
}

#[test]
fn rendered_unsupported_assignment_target_diagnostics_anchor_to_the_target() {
    let source = r#"
struct Counter {
    value: Int,
}

fn main() -> Int {
    var counter = Counter { value: 1 }
    counter.value = 2
    return counter.value
}
"#;
    let rendered = rendered_diagnostics(source);

    assert!(rendered.contains(
        "error: sample.ql:8:5: assignment through member access is not supported yet; only bare mutable bindings can be assigned"
    ));
}

#[test]
fn rendered_ambiguous_method_diagnostics_anchor_to_the_member_access() {
    let source = r#"
struct Counter {
    value: Int,
}

extend Counter {
    fn ping(self) -> Int {
        return self.value
    }
}

extend Counter {
    fn ping(self, delta: Int) -> Int {
        return self.value + delta
    }
}

fn main(counter: Counter) -> Int {
    return counter.ping()
}
"#;
    let rendered = rendered_diagnostics(source);

    assert!(rendered.contains(
        "error: sample.ql:19:12: ambiguous method `ping` on type `Counter`; multiple matching methods found"
    ));
}

#[test]
fn rendered_import_alias_call_diagnostics_anchor_to_the_use_site() {
    let source = r#"
use VALUE as current

const VALUE: Int = 1

fn main() -> Int {
    return current()
}
"#;
    let rendered = rendered_diagnostics(source);

    assert!(rendered.contains("error: sample.ql:7:12: cannot call value of type `Int`"));
}

#[test]
fn rendered_invalid_projection_receiver_diagnostics_anchor_to_the_projection_site() {
    let source = r#"
fn main() -> Int {
    let value = 1
    value.name;
    value[0];
    return 0
}
"#;
    let rendered = rendered_diagnostics(source);

    assert!(
        rendered.contains("error: sample.ql:4:5: member access is not supported on type `Int`")
    );
    assert!(rendered.contains(
        "error: sample.ql:5:5: indexing is not supported on type `Int`; only arrays and tuples are indexable"
    ));
}

#[test]
fn rendered_invalid_struct_literal_root_diagnostics_anchor_to_the_literal() {
    let source = r#"
enum Command {
    Value(Int),
}

fn main() -> Int {
    let value = Command.Value { field: 1 }
    return 0
}
"#;
    let rendered = rendered_diagnostics(source);

    assert!(rendered.contains(
        "error: sample.ql:7:17: struct literal syntax is not supported for `Command.Value`"
    ));
}

#[test]
fn rendered_invalid_struct_literal_roots_do_not_cascade_into_return_mismatches() {
    let source = r#"
enum Command {
    Value(Int),
}

fn main() -> Bool {
    return Command.Value { value: 1 }
}
"#;
    let rendered = rendered_diagnostics(source);

    assert!(rendered.contains(
        "error: sample.ql:7:12: struct literal syntax is not supported for `Command.Value`"
    ));
    assert!(!rendered.contains("return value has type mismatch"));
}

#[test]
fn rendered_invalid_generic_struct_literal_root_diagnostics_anchor_to_the_literal() {
    let source = r#"
fn build[T]() -> Int {
    let value = T { field: 1 }
    return 0
}
"#;
    let rendered = rendered_diagnostics(source);

    assert!(
        rendered.contains("error: sample.ql:3:17: struct literal syntax is not supported for `T`")
    );
}

#[test]
fn rendered_unknown_enum_variant_diagnostics_anchor_to_the_use_site() {
    let source = r#"
enum Command {
    Config {
        retries: Int,
    },
}

fn main() -> Command {
    return Command.Missing { retries: 1 }
}
"#;
    let rendered = rendered_diagnostics(source);

    assert!(
        rendered.contains("error: sample.ql:9:12: unknown variant `Missing` in enum `Command`")
    );
}

#[test]
fn rendered_invalid_pattern_root_diagnostics_anchor_to_the_pattern() {
    let source = r#"
struct Point {
    x: Int,
}

fn main(point: Point) -> Int {
    let Point(value) = point
    return 0
}
"#;
    let rendered = rendered_diagnostics(source);

    assert!(rendered.contains(
        "error: sample.ql:7:9: tuple-struct pattern syntax is not supported for `Point`"
    ));
}

#[test]
fn rendered_invalid_path_pattern_root_diagnostics_anchor_to_the_pattern() {
    let source = r#"
struct Point {
    x: Int,
}

fn main(point: Point) -> Int {
    let Point = point
    return 0
}
"#;
    let rendered = rendered_diagnostics(source);

    assert!(
        rendered.contains("error: sample.ql:7:9: path pattern syntax is not supported for `Point`")
    );
}

#[test]
fn rendered_unsupported_const_path_pattern_diagnostics_anchor_to_the_pattern() {
    let source = r#"
const LIMIT: Int = 1

fn main(value: Int) -> Int {
    match value {
        LIMIT => 1,
        _ => 0,
    }
}
"#;
    let rendered = rendered_diagnostics(source);

    assert!(
        rendered.contains("error: sample.ql:6:9: path pattern syntax is not supported for `LIMIT`")
    );
}
