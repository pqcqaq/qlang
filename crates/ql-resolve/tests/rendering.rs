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
