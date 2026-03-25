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
