mod support;

use ql_analysis::SymbolKind;

use support::{analyzed, nth_offset, nth_span};

#[test]
fn definition_queries_follow_generics_parameters_and_locals() {
    let source = r#"
fn id[T](param: T) -> T {
    let local_value = param
    local_value
}
"#;

    let analysis = analyzed(source);

    assert_eq!(
        analysis.definition_at(nth_offset(source, "T", 2)),
        Some(ql_analysis::DefinitionTarget {
            kind: SymbolKind::Generic,
            name: "T".to_owned(),
            span: nth_span(source, "T", 1),
        })
    );
    assert_eq!(
        analysis.definition_at(nth_offset(source, "param", 2)),
        Some(ql_analysis::DefinitionTarget {
            kind: SymbolKind::Parameter,
            name: "param".to_owned(),
            span: nth_span(source, "param", 1),
        })
    );
    assert_eq!(
        analysis.definition_at(nth_offset(source, "local_value", 2)),
        Some(ql_analysis::DefinitionTarget {
            kind: SymbolKind::Local,
            name: "local_value".to_owned(),
            span: nth_span(source, "local_value", 1),
        })
    );
}

#[test]
fn hover_queries_report_items_imports_and_builtins() {
    let source = r#"
use std.collections.HashMap as Map

struct Counter {
    value: Int,
}

fn build(cache: Map[String, Int]) -> Counter {
    Counter { value: 1 }
}
"#;

    let analysis = analyzed(source);

    let map_hover = analysis
        .hover_at(
            source
                .find("Map[String, Int]")
                .expect("parameter type should contain the import alias"),
        )
        .expect("import alias should hover");
    assert_eq!(map_hover.kind, SymbolKind::Import);
    assert_eq!(map_hover.detail, "import std.collections.HashMap");
    assert_eq!(map_hover.definition_span, None);

    let string_hover = analysis
        .hover_at(nth_offset(source, "String", 1))
        .expect("builtin type should hover");
    assert_eq!(string_hover.kind, SymbolKind::BuiltinType);
    assert_eq!(string_hover.detail, "builtin type String");
    assert_eq!(string_hover.definition_span, None);

    let counter_hover = analysis
        .hover_at(nth_offset(source, "Counter", 2))
        .expect("type use should hover");
    assert_eq!(counter_hover.kind, SymbolKind::Struct);
    assert_eq!(counter_hover.detail, "struct Counter");
    assert_eq!(
        counter_hover.definition_span,
        Some(nth_span(source, "Counter", 1))
    );

    let function_hover = analysis
        .hover_at(nth_offset(source, "build", 1))
        .expect("function declaration should hover");
    assert_eq!(function_hover.kind, SymbolKind::Function);
    assert_eq!(
        function_hover.detail,
        "fn build(cache: Map[String, Int]) -> Counter"
    );
}

#[test]
fn receiver_queries_stay_on_the_root_binding() {
    let source = r#"
struct Counter {
    value: Int,
}

impl Counter {
    fn read(self) -> Int {
        self.value
    }
}
"#;

    let analysis = analyzed(source);
    let self_hover = analysis
        .hover_at(nth_offset(source, "self", 2))
        .expect("receiver use should hover");

    assert_eq!(self_hover.kind, SymbolKind::SelfParameter);
    assert_eq!(self_hover.detail, "receiver self: Counter");
    assert_eq!(self_hover.ty.as_deref(), Some("Counter"));
    assert_eq!(
        self_hover.definition_span,
        Some(nth_span(source, "self", 1))
    );
    assert_eq!(
        analysis.definition_at(nth_offset(source, "self", 2)),
        Some(ql_analysis::DefinitionTarget {
            kind: SymbolKind::SelfParameter,
            name: "self".to_owned(),
            span: nth_span(source, "self", 1),
        })
    );

    assert_eq!(analysis.hover_at(nth_offset(source, "value", 2)), None);
}
