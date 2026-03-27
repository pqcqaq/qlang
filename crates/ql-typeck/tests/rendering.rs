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
