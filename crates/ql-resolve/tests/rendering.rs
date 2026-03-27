mod support;

use support::rendered_diagnostics;

#[test]
fn rendered_self_diagnostics_anchor_to_the_keyword_span() {
    let source = r#"
fn main() -> Int {
    self
}
"#;

    let rendered = rendered_diagnostics(source);

    assert!(
        rendered.contains(
            "error: sample.ql:3:5: invalid use of `self` outside a method receiver scope"
        )
    );
    assert!(rendered.contains("^^^^ `self` is only available inside methods"));
}

#[test]
fn rendered_unresolved_value_diagnostics_anchor_to_the_name_span() {
    let source = r#"
fn main() -> Int {
    missing
}
"#;

    let rendered = rendered_diagnostics(source);

    assert!(rendered.contains("error: sample.ql:3:5: unresolved value `missing`"));
    assert!(rendered.contains("^^^^^^^ could not resolve this value in the current scope"));
}

#[test]
fn rendered_unresolved_type_diagnostics_anchor_to_the_type_root_span() {
    let source = r#"
fn build(input: Missing) -> Int {
    0
}
"#;

    let rendered = rendered_diagnostics(source);

    assert!(rendered.contains("error: sample.ql:2:17: unresolved type `Missing`"));
    assert!(rendered.contains("^^^^^^^ could not resolve this type in the current scope"));
}
