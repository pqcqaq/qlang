use super::*;

#[test]
fn specialized_body_rewrites_apply_from_unordered_input_and_ignore_outside_spans() {
    let rewrites = [
        SourceRewrite {
            span: ql_span::Span::new(30, 36),
            replacement: "two".to_owned(),
        },
        SourceRewrite {
            span: ql_span::Span::new(20, 25),
            replacement: "one".to_owned(),
        },
        SourceRewrite {
            span: ql_span::Span::new(0, 5),
            replacement: "outside".to_owned(),
        },
    ];

    assert_eq!(
        apply_specialized_body_rewrites("first() + second()", 20, &rewrites),
        "one() + two()"
    );
}
