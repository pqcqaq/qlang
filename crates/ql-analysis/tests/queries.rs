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
fn reference_queries_follow_parameters_and_locals() {
    let source = r#"
fn id[T](param: T) -> T {
    let local_value = param
    local_value
}
"#;

    let analysis = analyzed(source);

    assert_eq!(
        analysis.references_at(nth_offset(source, "param", 2)),
        Some(vec![
            ql_analysis::ReferenceTarget {
                kind: SymbolKind::Parameter,
                name: "param".to_owned(),
                span: nth_span(source, "param", 1),
                is_definition: true,
            },
            ql_analysis::ReferenceTarget {
                kind: SymbolKind::Parameter,
                name: "param".to_owned(),
                span: nth_span(source, "param", 2),
                is_definition: false,
            },
        ])
    );
    assert_eq!(
        analysis.references_at(nth_offset(source, "local_value", 2)),
        Some(vec![
            ql_analysis::ReferenceTarget {
                kind: SymbolKind::Local,
                name: "local_value".to_owned(),
                span: nth_span(source, "local_value", 1),
                is_definition: true,
            },
            ql_analysis::ReferenceTarget {
                kind: SymbolKind::Local,
                name: "local_value".to_owned(),
                span: nth_span(source, "local_value", 2),
                is_definition: false,
            },
        ])
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
fn import_reference_queries_group_non_definition_uses() {
    let source = r#"
use std.collections.HashMap as Map

fn build(cache: Map[String, Int]) -> Map[String, Int] {
    return cache
}
"#;

    let analysis = analyzed(source);
    let first_use = source
        .find("Map[String, Int]")
        .expect("first import alias use should exist");
    let second_use = source
        .rfind("Map[String, Int]")
        .expect("second import alias use should exist");

    assert_eq!(
        analysis.references_at(first_use),
        Some(vec![
            ql_analysis::ReferenceTarget {
                kind: SymbolKind::Import,
                name: "HashMap".to_owned(),
                span: ql_span::Span::new(first_use, first_use + "Map".len()),
                is_definition: false,
            },
            ql_analysis::ReferenceTarget {
                kind: SymbolKind::Import,
                name: "HashMap".to_owned(),
                span: ql_span::Span::new(second_use, second_use + "Map".len()),
                is_definition: false,
            },
        ])
    );
}

#[test]
fn receiver_and_member_queries_follow_precise_symbols() {
    let source = r#"
struct Counter {
    value: Int,
}

impl Counter {
    fn get(self) -> Int {
        return self.value
    }

    fn read(self) -> Int {
        return self.get()
    }
}
"#;

    let analysis = analyzed(source);
    let self_use = source.find("self.get").expect("self.get should exist");
    let field_use = source
        .find(".value")
        .map(|offset| offset + 1)
        .expect("member field use should exist");
    let method_use = source
        .find(".get")
        .map(|offset| offset + 1)
        .expect("member method use should exist");
    let self_hover = analysis
        .hover_at(self_use)
        .expect("receiver use should hover");

    assert_eq!(self_hover.kind, SymbolKind::SelfParameter);
    assert_eq!(self_hover.detail, "receiver self: Counter");
    assert_eq!(self_hover.ty.as_deref(), Some("Counter"));
    assert_eq!(
        self_hover.definition_span,
        Some(nth_span(source, "self", 3))
    );
    assert_eq!(
        analysis.definition_at(self_use),
        Some(ql_analysis::DefinitionTarget {
            kind: SymbolKind::SelfParameter,
            name: "self".to_owned(),
            span: nth_span(source, "self", 3),
        })
    );

    let field_hover = analysis
        .hover_at(field_use)
        .expect("member field use should hover");
    assert_eq!(field_hover.kind, SymbolKind::Field);
    assert_eq!(field_hover.detail, "field value: Int");
    assert_eq!(field_hover.ty.as_deref(), Some("Int"));
    assert_eq!(
        field_hover.definition_span,
        Some(nth_span(source, "value", 1))
    );
    assert_eq!(
        analysis.definition_at(field_use),
        Some(ql_analysis::DefinitionTarget {
            kind: SymbolKind::Field,
            name: "value".to_owned(),
            span: nth_span(source, "value", 1),
        })
    );
    assert_eq!(
        analysis.references_at(field_use),
        Some(vec![
            ql_analysis::ReferenceTarget {
                kind: SymbolKind::Field,
                name: "value".to_owned(),
                span: nth_span(source, "value", 1),
                is_definition: true,
            },
            ql_analysis::ReferenceTarget {
                kind: SymbolKind::Field,
                name: "value".to_owned(),
                span: nth_span(source, "value", 2),
                is_definition: false,
            },
        ])
    );

    let method_hover = analysis
        .hover_at(method_use)
        .expect("method use should hover");
    assert_eq!(method_hover.kind, SymbolKind::Method);
    assert_eq!(method_hover.detail, "fn get(self) -> Int");
    assert_eq!(
        method_hover.definition_span,
        Some(nth_span(source, "get", 1))
    );
    assert_eq!(
        analysis.definition_at(method_use),
        Some(ql_analysis::DefinitionTarget {
            kind: SymbolKind::Method,
            name: "get".to_owned(),
            span: nth_span(source, "get", 1),
        })
    );
    assert_eq!(
        analysis.references_at(method_use),
        Some(vec![
            ql_analysis::ReferenceTarget {
                kind: SymbolKind::Method,
                name: "get".to_owned(),
                span: nth_span(source, "get", 1),
                is_definition: true,
            },
            ql_analysis::ReferenceTarget {
                kind: SymbolKind::Method,
                name: "get".to_owned(),
                span: nth_span(source, "get", 2),
                is_definition: false,
            },
        ])
    );
}

#[test]
fn member_queries_prefer_impl_methods_over_extend_methods() {
    let source = r#"
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
    return counter.read(1)
}
"#;

    let analysis = analyzed(source);
    let method_use = source
        .rfind(".read")
        .map(|offset| offset + 1)
        .expect("member method use should exist");

    let hover = analysis
        .hover_at(method_use)
        .expect("member method hover should exist");
    assert_eq!(hover.kind, SymbolKind::Method);
    assert_eq!(hover.detail, "fn read(self, delta: Int) -> Int");
    assert_eq!(hover.definition_span, Some(nth_span(source, "read", 1)));
    assert_eq!(
        analysis.definition_at(method_use),
        Some(ql_analysis::DefinitionTarget {
            kind: SymbolKind::Method,
            name: "read".to_owned(),
            span: nth_span(source, "read", 1),
        })
    );
}

#[test]
fn variant_queries_follow_declarations_patterns_and_constructors() {
    let source = r#"
enum Command {
    Retry(Int),
    Config { retries: Int },
}

fn build(flag: Bool) -> Command {
    if flag {
        return Command.Retry(1)
    }
    return Command.Config { retries: 2 }
}

fn read(command: Command) -> Int {
    match command {
        Command.Retry(times) => times,
        Command.Config { retries } => retries,
    }
}
"#;

    let analysis = analyzed(source);
    let retry_use = source
        .find(".Retry")
        .map(|offset| offset + 1)
        .expect("retry constructor should exist");
    let config_literal_use = source
        .find("Command.Config {")
        .map(|offset| offset + "Command.".len())
        .expect("config struct literal should exist");
    let config_pattern_use = source
        .rfind("Command.Config {")
        .map(|offset| offset + "Command.".len())
        .expect("config pattern should exist");

    let retry_hover = analysis
        .hover_at(retry_use)
        .expect("retry variant hover should exist");
    assert_eq!(retry_hover.kind, SymbolKind::Variant);
    assert_eq!(retry_hover.detail, "variant Command.Retry(Int)");
    assert_eq!(retry_hover.ty.as_deref(), Some("Command"));
    assert_eq!(
        retry_hover.definition_span,
        Some(nth_span(source, "Retry", 1))
    );
    assert_eq!(
        analysis.definition_at(retry_use),
        Some(ql_analysis::DefinitionTarget {
            kind: SymbolKind::Variant,
            name: "Retry".to_owned(),
            span: nth_span(source, "Retry", 1),
        })
    );
    assert_eq!(
        analysis.references_at(retry_use),
        Some(vec![
            ql_analysis::ReferenceTarget {
                kind: SymbolKind::Variant,
                name: "Retry".to_owned(),
                span: nth_span(source, "Retry", 1),
                is_definition: true,
            },
            ql_analysis::ReferenceTarget {
                kind: SymbolKind::Variant,
                name: "Retry".to_owned(),
                span: nth_span(source, "Retry", 2),
                is_definition: false,
            },
            ql_analysis::ReferenceTarget {
                kind: SymbolKind::Variant,
                name: "Retry".to_owned(),
                span: nth_span(source, "Retry", 3),
                is_definition: false,
            },
        ])
    );

    let config_hover = analysis
        .hover_at(config_literal_use)
        .expect("config variant hover should exist");
    assert_eq!(config_hover.kind, SymbolKind::Variant);
    assert_eq!(
        config_hover.detail,
        "variant Command.Config { retries: Int }"
    );
    assert_eq!(
        config_hover.definition_span,
        Some(nth_span(source, "Config", 1))
    );
    assert_eq!(
        analysis.definition_at(config_pattern_use),
        Some(ql_analysis::DefinitionTarget {
            kind: SymbolKind::Variant,
            name: "Config".to_owned(),
            span: nth_span(source, "Config", 1),
        })
    );
    assert_eq!(
        analysis.references_at(config_literal_use),
        Some(vec![
            ql_analysis::ReferenceTarget {
                kind: SymbolKind::Variant,
                name: "Config".to_owned(),
                span: nth_span(source, "Config", 1),
                is_definition: true,
            },
            ql_analysis::ReferenceTarget {
                kind: SymbolKind::Variant,
                name: "Config".to_owned(),
                span: nth_span(source, "Config", 2),
                is_definition: false,
            },
            ql_analysis::ReferenceTarget {
                kind: SymbolKind::Variant,
                name: "Config".to_owned(),
                span: nth_span(source, "Config", 3),
                is_definition: false,
            },
        ])
    );
}

#[test]
fn rename_queries_follow_supported_same_file_symbols() {
    let source = r#"
enum Command {
    Retry(Int),
}

fn id[T](value: T) -> T {
    let local_value = value
    return local_value
}

fn build() -> Command {
    return Command.Retry(id(1))
}
"#;

    let analysis = analyzed(source);
    let param_use = source
        .find("= value")
        .map(|offset| offset + 2)
        .expect("parameter use should exist");
    let param_use_span = ql_span::Span::new(param_use, param_use + "value".len());

    assert_eq!(
        analysis.prepare_rename_at(nth_offset(source, "T", 2)),
        Some(ql_analysis::RenameTarget {
            kind: SymbolKind::Generic,
            name: "T".to_owned(),
            span: nth_span(source, "T", 2),
        })
    );
    assert_eq!(
        analysis.rename_at(param_use, "input"),
        Ok(Some(ql_analysis::RenameResult {
            kind: SymbolKind::Parameter,
            old_name: "value".to_owned(),
            new_name: "input".to_owned(),
            edits: vec![
                ql_analysis::RenameEdit {
                    span: nth_span(source, "value", 1),
                    replacement: "input".to_owned(),
                },
                ql_analysis::RenameEdit {
                    span: param_use_span,
                    replacement: "input".to_owned(),
                },
            ],
        }))
    );
    assert_eq!(
        analysis.rename_at(nth_offset(source, "local_value", 2), "result"),
        Ok(Some(ql_analysis::RenameResult {
            kind: SymbolKind::Local,
            old_name: "local_value".to_owned(),
            new_name: "result".to_owned(),
            edits: vec![
                ql_analysis::RenameEdit {
                    span: nth_span(source, "local_value", 1),
                    replacement: "result".to_owned(),
                },
                ql_analysis::RenameEdit {
                    span: nth_span(source, "local_value", 2),
                    replacement: "result".to_owned(),
                },
            ],
        }))
    );
    assert_eq!(
        analysis.rename_at(nth_offset(source, "id", 2), "identity"),
        Ok(Some(ql_analysis::RenameResult {
            kind: SymbolKind::Function,
            old_name: "id".to_owned(),
            new_name: "identity".to_owned(),
            edits: vec![
                ql_analysis::RenameEdit {
                    span: nth_span(source, "id", 1),
                    replacement: "identity".to_owned(),
                },
                ql_analysis::RenameEdit {
                    span: nth_span(source, "id", 2),
                    replacement: "identity".to_owned(),
                },
            ],
        }))
    );
    assert_eq!(
        analysis.rename_at(nth_offset(source, "Retry", 2), "Repeat"),
        Ok(Some(ql_analysis::RenameResult {
            kind: SymbolKind::Variant,
            old_name: "Retry".to_owned(),
            new_name: "Repeat".to_owned(),
            edits: vec![
                ql_analysis::RenameEdit {
                    span: nth_span(source, "Retry", 1),
                    replacement: "Repeat".to_owned(),
                },
                ql_analysis::RenameEdit {
                    span: nth_span(source, "Retry", 2),
                    replacement: "Repeat".to_owned(),
                },
            ],
        }))
    );
}

#[test]
fn rename_queries_validate_new_identifiers() {
    let source = r#"
fn id(value: Int) -> Int {
    return value
}
"#;

    let analysis = analyzed(source);
    let value_use = nth_offset(source, "value", 2);

    assert_eq!(
        analysis.rename_at(value_use, "match"),
        Err(ql_analysis::RenameError::Keyword("match".to_owned()))
    );
    assert_eq!(
        analysis.rename_at(value_use, "2value"),
        Err(ql_analysis::RenameError::InvalidIdentifier(
            "2value".to_owned()
        ))
    );

    let escaped = analysis
        .rename_at(value_use, "`match`")
        .expect("escaped keywords should validate")
        .expect("parameter rename should be supported");
    assert_eq!(escaped.new_name, "`match`");
    assert_eq!(escaped.edits.len(), 2);
    assert!(
        escaped
            .edits
            .iter()
            .all(|edit| edit.replacement == "`match`")
    );
}

#[test]
fn rename_queries_skip_unsupported_symbols() {
    let source = r#"
use std.collections.HashMap as Map

struct Counter {
    value: Int,
}

impl Counter {
    fn get(self, cache: Map[String, Int]) -> Int {
        return self.value + self.get()
    }
}
"#;

    let analysis = analyzed(source);
    let import_use = source
        .find("Map[String, Int]")
        .expect("import alias use should exist");
    let builtin_use = nth_offset(source, "String", 1);
    let field_use = source
        .find(".value")
        .map(|offset| offset + 1)
        .expect("field use should exist");
    let method_use = source
        .find(".get")
        .map(|offset| offset + 1)
        .expect("method use should exist");
    let self_use = source.find("self.value").expect("self use should exist");

    assert_eq!(analysis.prepare_rename_at(import_use), None);
    assert_eq!(analysis.prepare_rename_at(builtin_use), None);
    assert_eq!(analysis.prepare_rename_at(field_use), None);
    assert_eq!(analysis.prepare_rename_at(method_use), None);
    assert_eq!(analysis.prepare_rename_at(self_use), None);

    assert_eq!(analysis.rename_at(field_use, "renamed"), Ok(None));
    assert_eq!(analysis.rename_at(method_use, "renamed"), Ok(None));
}

#[test]
fn extern_block_function_queries_follow_callable_declarations() {
    let source = r#"
extern "c" {
    fn q_add(left: Int, right: Int) -> Int
}

fn main() -> Int {
    return q_add(1, 2)
}
"#;

    let analysis = analyzed(source);
    let hover = analysis
        .hover_at(nth_offset(source, "q_add", 2))
        .expect("extern call should hover");

    assert_eq!(hover.kind, SymbolKind::Function);
    assert_eq!(
        hover.detail,
        "extern \"c\" fn q_add(left: Int, right: Int) -> Int"
    );
    assert_eq!(hover.definition_span, Some(nth_span(source, "q_add", 1)));
    assert_eq!(
        analysis.definition_at(nth_offset(source, "q_add", 2)),
        Some(ql_analysis::DefinitionTarget {
            kind: SymbolKind::Function,
            name: "q_add".to_owned(),
            span: nth_span(source, "q_add", 1),
        })
    );
    assert_eq!(
        analysis.references_at(nth_offset(source, "q_add", 2)),
        Some(vec![
            ql_analysis::ReferenceTarget {
                kind: SymbolKind::Function,
                name: "q_add".to_owned(),
                span: nth_span(source, "q_add", 1),
                is_definition: true,
            },
            ql_analysis::ReferenceTarget {
                kind: SymbolKind::Function,
                name: "q_add".to_owned(),
                span: nth_span(source, "q_add", 2),
                is_definition: false,
            },
        ])
    );
}
