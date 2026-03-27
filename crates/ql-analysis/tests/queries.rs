mod support;

use ql_analysis::SymbolKind;
use ql_span::Span;

use support::{analyzed, nth_offset, nth_span};

fn alias_span(source: &str, alias: &str) -> Span {
    source
        .find(&format!("as {alias}"))
        .map(|offset| Span::new(offset + 3, offset + 3 + alias.len()))
        .expect("import alias definition should exist")
}

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
fn lexical_semantic_symbol_queries_follow_same_file_identity() {
    let source = r#"
fn id[T](param: T) -> T {
    let local_value = param
    return local_value
}

struct Counter {
    value: String,
}

impl Counter {
    fn read(self, input: String) -> String {
        let alias = input
        return self.value
    }
}
"#;

    let analysis = analyzed(source);
    let generic_use = nth_offset(source, "T", 3);
    let parameter_use = nth_offset(source, "param", 2);
    let local_use = nth_offset(source, "local_value", 2);
    let self_use = source.find("self.value").expect("self use should exist");
    let builtin_use = nth_offset(source, "String", 2);

    let generic_hover = analysis
        .hover_at(generic_use)
        .expect("generic hover should exist");
    assert_eq!(generic_hover.kind, SymbolKind::Generic);
    assert_eq!(generic_hover.detail, "generic T");
    assert_eq!(
        generic_hover.definition_span,
        Some(nth_span(source, "T", 1))
    );
    assert_eq!(
        analysis.definition_at(generic_use),
        Some(ql_analysis::DefinitionTarget {
            kind: SymbolKind::Generic,
            name: "T".to_owned(),
            span: nth_span(source, "T", 1),
        })
    );
    assert_eq!(
        analysis.references_at(generic_use),
        Some(vec![
            ql_analysis::ReferenceTarget {
                kind: SymbolKind::Generic,
                name: "T".to_owned(),
                span: nth_span(source, "T", 1),
                is_definition: true,
            },
            ql_analysis::ReferenceTarget {
                kind: SymbolKind::Generic,
                name: "T".to_owned(),
                span: nth_span(source, "T", 2),
                is_definition: false,
            },
            ql_analysis::ReferenceTarget {
                kind: SymbolKind::Generic,
                name: "T".to_owned(),
                span: nth_span(source, "T", 3),
                is_definition: false,
            },
        ])
    );

    let parameter_hover = analysis
        .hover_at(parameter_use)
        .expect("parameter hover should exist");
    assert_eq!(parameter_hover.kind, SymbolKind::Parameter);
    assert_eq!(parameter_hover.detail, "param param: T");
    assert_eq!(parameter_hover.ty.as_deref(), Some("T"));
    assert_eq!(
        parameter_hover.definition_span,
        Some(nth_span(source, "param", 1))
    );
    assert_eq!(
        analysis.definition_at(parameter_use),
        Some(ql_analysis::DefinitionTarget {
            kind: SymbolKind::Parameter,
            name: "param".to_owned(),
            span: nth_span(source, "param", 1),
        })
    );
    assert_eq!(
        analysis.references_at(parameter_use),
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

    let local_hover = analysis
        .hover_at(local_use)
        .expect("local hover should exist");
    assert_eq!(local_hover.kind, SymbolKind::Local);
    assert_eq!(local_hover.detail, "local local_value: T");
    assert_eq!(local_hover.ty.as_deref(), Some("T"));
    assert_eq!(
        local_hover.definition_span,
        Some(nth_span(source, "local_value", 1))
    );
    assert_eq!(
        analysis.definition_at(local_use),
        Some(ql_analysis::DefinitionTarget {
            kind: SymbolKind::Local,
            name: "local_value".to_owned(),
            span: nth_span(source, "local_value", 1),
        })
    );
    assert_eq!(
        analysis.references_at(local_use),
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

    let self_hover = analysis
        .hover_at(self_use)
        .expect("receiver hover should exist");
    assert_eq!(self_hover.kind, SymbolKind::SelfParameter);
    assert_eq!(self_hover.detail, "receiver self: Counter");
    assert_eq!(self_hover.ty.as_deref(), Some("Counter"));
    assert_eq!(
        self_hover.definition_span,
        Some(nth_span(source, "self", 1))
    );
    assert_eq!(
        analysis.definition_at(self_use),
        Some(ql_analysis::DefinitionTarget {
            kind: SymbolKind::SelfParameter,
            name: "self".to_owned(),
            span: nth_span(source, "self", 1),
        })
    );
    assert_eq!(
        analysis.references_at(self_use),
        Some(vec![
            ql_analysis::ReferenceTarget {
                kind: SymbolKind::SelfParameter,
                name: "self".to_owned(),
                span: nth_span(source, "self", 1),
                is_definition: true,
            },
            ql_analysis::ReferenceTarget {
                kind: SymbolKind::SelfParameter,
                name: "self".to_owned(),
                span: nth_span(source, "self", 2),
                is_definition: false,
            },
        ])
    );

    let builtin_hover = analysis
        .hover_at(builtin_use)
        .expect("builtin type hover should exist");
    assert_eq!(builtin_hover.kind, SymbolKind::BuiltinType);
    assert_eq!(builtin_hover.detail, "builtin type String");
    assert_eq!(builtin_hover.definition_span, None);
    assert_eq!(analysis.definition_at(builtin_use), None);
    assert_eq!(
        analysis.references_at(builtin_use),
        Some(vec![
            ql_analysis::ReferenceTarget {
                kind: SymbolKind::BuiltinType,
                name: "String".to_owned(),
                span: nth_span(source, "String", 1),
                is_definition: false,
            },
            ql_analysis::ReferenceTarget {
                kind: SymbolKind::BuiltinType,
                name: "String".to_owned(),
                span: nth_span(source, "String", 2),
                is_definition: false,
            },
            ql_analysis::ReferenceTarget {
                kind: SymbolKind::BuiltinType,
                name: "String".to_owned(),
                span: nth_span(source, "String", 3),
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
    assert_eq!(map_hover.name, "Map");
    assert_eq!(map_hover.definition_span, Some(alias_span(source, "Map")));

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
fn hover_queries_render_array_type_signatures() {
    let source = r#"
fn take(values: [Int; 3]) -> [Int; 3] {
    return values
}

fn main() -> Int {
    return take([1, 2, 3])[0]
}
"#;

    let analysis = analyzed(source);
    let hover = analysis
        .hover_at(nth_offset(source, "take", 2))
        .expect("array-typed function use should hover");

    assert_eq!(hover.kind, SymbolKind::Function);
    assert_eq!(hover.detail, "fn take(values: [Int; 3]) -> [Int; 3]");
    assert_eq!(hover.definition_span, Some(nth_span(source, "take", 1)));
}

#[test]
fn free_function_queries_follow_same_file_identity() {
    let source = r#"
fn helper(value: Int) -> Int {
    return value
}

fn compute() -> Int {
    return helper(1) + helper(2)
}
"#;

    let analysis = analyzed(source);
    let helper_use = nth_offset(source, "helper", 2);

    let hover = analysis
        .hover_at(helper_use)
        .expect("free function use should hover");
    assert_eq!(hover.kind, SymbolKind::Function);
    assert_eq!(hover.detail, "fn helper(value: Int) -> Int");
    assert_eq!(hover.definition_span, Some(nth_span(source, "helper", 1)));

    assert_eq!(
        analysis.definition_at(helper_use),
        Some(ql_analysis::DefinitionTarget {
            kind: SymbolKind::Function,
            name: "helper".to_owned(),
            span: nth_span(source, "helper", 1),
        })
    );
    assert_eq!(
        analysis.references_at(helper_use),
        Some(vec![
            ql_analysis::ReferenceTarget {
                kind: SymbolKind::Function,
                name: "helper".to_owned(),
                span: nth_span(source, "helper", 1),
                is_definition: true,
            },
            ql_analysis::ReferenceTarget {
                kind: SymbolKind::Function,
                name: "helper".to_owned(),
                span: nth_span(source, "helper", 2),
                is_definition: false,
            },
            ql_analysis::ReferenceTarget {
                kind: SymbolKind::Function,
                name: "helper".to_owned(),
                span: nth_span(source, "helper", 3),
                is_definition: false,
            },
        ])
    );
}

#[test]
fn opaque_type_queries_follow_type_namespace_item_symbols() {
    let source = r#"
opaque type UserId = Int

struct Account {
    id: UserId,
}

fn build(value: UserId) -> UserId {
    return value
}
"#;

    let analysis = analyzed(source);
    let opaque_use = source
        .find("id: UserId")
        .map(|offset| offset + "id: ".len())
        .expect("opaque type use should exist");

    let hover = analysis
        .hover_at(opaque_use)
        .expect("opaque type hover should exist");
    assert_eq!(hover.kind, SymbolKind::TypeAlias);
    assert_eq!(hover.detail, "opaque type UserId = Int");
    assert_eq!(hover.definition_span, Some(nth_span(source, "UserId", 1)));
    assert_eq!(
        analysis.definition_at(opaque_use),
        Some(ql_analysis::DefinitionTarget {
            kind: SymbolKind::TypeAlias,
            name: "UserId".to_owned(),
            span: nth_span(source, "UserId", 1),
        })
    );
    assert_eq!(
        analysis.references_at(opaque_use),
        Some(vec![
            ql_analysis::ReferenceTarget {
                kind: SymbolKind::TypeAlias,
                name: "UserId".to_owned(),
                span: nth_span(source, "UserId", 1),
                is_definition: true,
            },
            ql_analysis::ReferenceTarget {
                kind: SymbolKind::TypeAlias,
                name: "UserId".to_owned(),
                span: nth_span(source, "UserId", 2),
                is_definition: false,
            },
            ql_analysis::ReferenceTarget {
                kind: SymbolKind::TypeAlias,
                name: "UserId".to_owned(),
                span: nth_span(source, "UserId", 3),
                is_definition: false,
            },
            ql_analysis::ReferenceTarget {
                kind: SymbolKind::TypeAlias,
                name: "UserId".to_owned(),
                span: nth_span(source, "UserId", 4),
                is_definition: false,
            },
        ])
    );
}

#[test]
fn hover_and_definition_queries_follow_type_namespace_item_symbols() {
    let source = r#"
type IdAlias = Int

struct Account {
    id: IdAlias,
}

enum Mode {
    Ready,
}

trait Taggable {
    fn mode(self) -> Mode
}

impl Taggable for Account {
    fn mode(self) -> Mode {
        return Mode.Ready
    }
}

fn build(account: Account, value: IdAlias) -> Mode {
    let copy = value
    return account.mode()
}
"#;

    let analysis = analyzed(source);
    let type_alias_use = source
        .find("id: IdAlias")
        .map(|offset| offset + "id: ".len())
        .expect("type alias use should exist");
    let struct_use = source
        .find("for Account")
        .map(|offset| offset + "for ".len())
        .expect("struct use should exist");
    let enum_use = source
        .rfind("-> Mode")
        .map(|offset| offset + 3)
        .expect("enum use should exist");
    let trait_use = source
        .find("impl Taggable")
        .map(|offset| offset + "impl ".len())
        .expect("trait use should exist");

    let type_alias_hover = analysis
        .hover_at(type_alias_use)
        .expect("type alias hover should exist");
    assert_eq!(type_alias_hover.kind, SymbolKind::TypeAlias);
    assert_eq!(type_alias_hover.detail, "type IdAlias = Int");
    assert_eq!(
        type_alias_hover.definition_span,
        Some(nth_span(source, "IdAlias", 1))
    );
    assert_eq!(
        analysis.definition_at(type_alias_use),
        Some(ql_analysis::DefinitionTarget {
            kind: SymbolKind::TypeAlias,
            name: "IdAlias".to_owned(),
            span: nth_span(source, "IdAlias", 1),
        })
    );

    let struct_hover = analysis
        .hover_at(struct_use)
        .expect("struct hover should exist");
    assert_eq!(struct_hover.kind, SymbolKind::Struct);
    assert_eq!(struct_hover.detail, "struct Account");
    assert_eq!(
        struct_hover.definition_span,
        Some(nth_span(source, "Account", 1))
    );
    assert_eq!(
        analysis.definition_at(struct_use),
        Some(ql_analysis::DefinitionTarget {
            kind: SymbolKind::Struct,
            name: "Account".to_owned(),
            span: nth_span(source, "Account", 1),
        })
    );

    let enum_hover = analysis
        .hover_at(enum_use)
        .expect("enum hover should exist");
    assert_eq!(enum_hover.kind, SymbolKind::Enum);
    assert_eq!(enum_hover.detail, "enum Mode");
    assert_eq!(
        enum_hover.definition_span,
        Some(nth_span(source, "Mode", 1))
    );
    assert_eq!(
        analysis.definition_at(enum_use),
        Some(ql_analysis::DefinitionTarget {
            kind: SymbolKind::Enum,
            name: "Mode".to_owned(),
            span: nth_span(source, "Mode", 1),
        })
    );

    let trait_hover = analysis
        .hover_at(trait_use)
        .expect("trait hover should exist");
    assert_eq!(trait_hover.kind, SymbolKind::Trait);
    assert_eq!(trait_hover.detail, "trait Taggable");
    assert_eq!(
        trait_hover.definition_span,
        Some(nth_span(source, "Taggable", 1))
    );
    assert_eq!(
        analysis.definition_at(trait_use),
        Some(ql_analysis::DefinitionTarget {
            kind: SymbolKind::Trait,
            name: "Taggable".to_owned(),
            span: nth_span(source, "Taggable", 1),
        })
    );
}

#[test]
fn global_value_item_queries_follow_same_file_identity() {
    let source = r#"
const LIMIT: Int = 10

static CURRENT: Int = LIMIT

fn read() -> Int {
    let snapshot = CURRENT
    return LIMIT
}
"#;

    let analysis = analyzed(source);
    let const_use = source
        .rfind("return LIMIT")
        .map(|offset| offset + "return ".len())
        .expect("const use should exist");
    let static_use = source
        .find("= CURRENT")
        .map(|offset| offset + 2)
        .expect("static use should exist");

    let const_hover = analysis
        .hover_at(const_use)
        .expect("const hover should exist");
    assert_eq!(const_hover.kind, SymbolKind::Const);
    assert_eq!(const_hover.detail, "const LIMIT: Int");
    assert_eq!(const_hover.ty.as_deref(), Some("Int"));
    assert_eq!(
        const_hover.definition_span,
        Some(nth_span(source, "LIMIT", 1))
    );
    assert_eq!(
        analysis.definition_at(const_use),
        Some(ql_analysis::DefinitionTarget {
            kind: SymbolKind::Const,
            name: "LIMIT".to_owned(),
            span: nth_span(source, "LIMIT", 1),
        })
    );
    assert_eq!(
        analysis.references_at(const_use),
        Some(vec![
            ql_analysis::ReferenceTarget {
                kind: SymbolKind::Const,
                name: "LIMIT".to_owned(),
                span: nth_span(source, "LIMIT", 1),
                is_definition: true,
            },
            ql_analysis::ReferenceTarget {
                kind: SymbolKind::Const,
                name: "LIMIT".to_owned(),
                span: nth_span(source, "LIMIT", 2),
                is_definition: false,
            },
            ql_analysis::ReferenceTarget {
                kind: SymbolKind::Const,
                name: "LIMIT".to_owned(),
                span: nth_span(source, "LIMIT", 3),
                is_definition: false,
            },
        ])
    );

    let static_hover = analysis
        .hover_at(static_use)
        .expect("static hover should exist");
    assert_eq!(static_hover.kind, SymbolKind::Static);
    assert_eq!(static_hover.detail, "static CURRENT: Int");
    assert_eq!(static_hover.ty.as_deref(), Some("Int"));
    assert_eq!(
        static_hover.definition_span,
        Some(nth_span(source, "CURRENT", 1))
    );
    assert_eq!(
        analysis.definition_at(static_use),
        Some(ql_analysis::DefinitionTarget {
            kind: SymbolKind::Static,
            name: "CURRENT".to_owned(),
            span: nth_span(source, "CURRENT", 1),
        })
    );
    assert_eq!(
        analysis.references_at(static_use),
        Some(vec![
            ql_analysis::ReferenceTarget {
                kind: SymbolKind::Static,
                name: "CURRENT".to_owned(),
                span: nth_span(source, "CURRENT", 1),
                is_definition: true,
            },
            ql_analysis::ReferenceTarget {
                kind: SymbolKind::Static,
                name: "CURRENT".to_owned(),
                span: nth_span(source, "CURRENT", 2),
                is_definition: false,
            },
        ])
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
                name: "Map".to_owned(),
                span: alias_span(source, "Map"),
                is_definition: true,
            },
            ql_analysis::ReferenceTarget {
                kind: SymbolKind::Import,
                name: "Map".to_owned(),
                span: ql_span::Span::new(first_use, first_use + "Map".len()),
                is_definition: false,
            },
            ql_analysis::ReferenceTarget {
                kind: SymbolKind::Import,
                name: "Map".to_owned(),
                span: ql_span::Span::new(second_use, second_use + "Map".len()),
                is_definition: false,
            },
        ])
    );
}

#[test]
fn import_alias_queries_follow_same_file_identity() {
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

    let hover = analysis
        .hover_at(first_use)
        .expect("import alias hover should exist");
    assert_eq!(hover.kind, SymbolKind::Import);
    assert_eq!(hover.detail, "import std.collections.HashMap");
    assert_eq!(hover.name, "Map");
    assert_eq!(hover.definition_span, Some(alias_span(source, "Map")));

    assert_eq!(
        analysis.definition_at(first_use),
        Some(ql_analysis::DefinitionTarget {
            kind: SymbolKind::Import,
            name: "Map".to_owned(),
            span: alias_span(source, "Map"),
        })
    );

    assert_eq!(
        analysis.references_at(first_use),
        Some(vec![
            ql_analysis::ReferenceTarget {
                kind: SymbolKind::Import,
                name: "Map".to_owned(),
                span: alias_span(source, "Map"),
                is_definition: true,
            },
            ql_analysis::ReferenceTarget {
                kind: SymbolKind::Import,
                name: "Map".to_owned(),
                span: ql_span::Span::new(first_use, first_use + "Map".len()),
                is_definition: false,
            },
            ql_analysis::ReferenceTarget {
                kind: SymbolKind::Import,
                name: "Map".to_owned(),
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
fn direct_member_queries_follow_same_file_surface_aggregate() {
    struct MemberCase<'a> {
        kind: SymbolKind,
        name: &'a str,
        use_occurrence: usize,
        detail: &'a str,
        reference_occurrences: &'a [usize],
    }

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
    let cases = [
        MemberCase {
            kind: SymbolKind::Field,
            name: "value",
            use_occurrence: 2,
            detail: "field value: Int",
            reference_occurrences: &[1, 2],
        },
        MemberCase {
            kind: SymbolKind::Method,
            name: "get",
            use_occurrence: 2,
            detail: "fn get(self) -> Int",
            reference_occurrences: &[1, 2],
        },
    ];

    for case in cases {
        let use_offset = nth_offset(source, case.name, case.use_occurrence);
        let hover = analysis
            .hover_at(use_offset)
            .expect("direct member hover should exist");

        assert_eq!(hover.kind, case.kind, "{}", case.name);
        assert_eq!(hover.detail, case.detail, "{}", case.name);
        assert_eq!(
            hover.definition_span,
            Some(nth_span(source, case.name, 1)),
            "{}",
            case.name
        );
        assert_eq!(
            analysis.definition_at(use_offset),
            Some(ql_analysis::DefinitionTarget {
                kind: case.kind,
                name: case.name.to_owned(),
                span: nth_span(source, case.name, 1),
            }),
            "{}",
            case.name
        );
        assert_eq!(
            analysis.references_at(use_offset),
            Some(
                case.reference_occurrences
                    .iter()
                    .map(|occurrence| ql_analysis::ReferenceTarget {
                        kind: case.kind,
                        name: case.name.to_owned(),
                        span: nth_span(source, case.name, *occurrence),
                        is_definition: *occurrence == 1,
                    })
                    .collect::<Vec<_>>()
            ),
            "{}",
            case.name
        );
    }
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
    assert_eq!(
        analysis.references_at(method_use),
        Some(vec![
            ql_analysis::ReferenceTarget {
                kind: SymbolKind::Method,
                name: "read".to_owned(),
                span: nth_span(source, "read", 1),
                is_definition: true,
            },
            ql_analysis::ReferenceTarget {
                kind: SymbolKind::Method,
                name: "read".to_owned(),
                span: nth_span(source, "read", 3),
                is_definition: false,
            },
        ])
    );
}

#[test]
fn explicit_struct_field_labels_join_field_queries() {
    let source = r#"
struct Point {
    x: Int,
    y: Int,
}

fn read(point: Point, value: Int) -> Int {
    let built = Point { x: value, y: 1 }
    match point {
        Point { x: alias, y: 2 } => alias,
    }
    return point.x
}
"#;

    let analysis = analyzed(source);
    let literal_field_x = source
        .find("{ x: value")
        .map(|offset| offset + 2)
        .expect("explicit struct literal field should exist");
    let pattern_field_x = source
        .find("{ x: alias")
        .map(|offset| offset + 2)
        .expect("explicit struct pattern field should exist");

    let hover = analysis
        .hover_at(literal_field_x)
        .expect("explicit field label should hover");
    assert_eq!(hover.kind, SymbolKind::Field);
    assert_eq!(hover.detail, "field x: Int");
    assert_eq!(hover.ty.as_deref(), Some("Int"));
    assert_eq!(hover.definition_span, Some(nth_span(source, "x", 1)));

    assert_eq!(
        analysis.definition_at(pattern_field_x),
        Some(ql_analysis::DefinitionTarget {
            kind: SymbolKind::Field,
            name: "x".to_owned(),
            span: nth_span(source, "x", 1),
        })
    );
    assert_eq!(
        analysis.references_at(literal_field_x),
        Some(vec![
            ql_analysis::ReferenceTarget {
                kind: SymbolKind::Field,
                name: "x".to_owned(),
                span: nth_span(source, "x", 1),
                is_definition: true,
            },
            ql_analysis::ReferenceTarget {
                kind: SymbolKind::Field,
                name: "x".to_owned(),
                span: nth_span(source, "x", 2),
                is_definition: false,
            },
            ql_analysis::ReferenceTarget {
                kind: SymbolKind::Field,
                name: "x".to_owned(),
                span: nth_span(source, "x", 3),
                is_definition: false,
            },
            ql_analysis::ReferenceTarget {
                kind: SymbolKind::Field,
                name: "x".to_owned(),
                span: nth_span(source, "x", 4),
                is_definition: false,
            },
        ])
    );
    assert_eq!(
        analysis.prepare_rename_at(literal_field_x),
        Some(ql_analysis::RenameTarget {
            kind: SymbolKind::Field,
            name: "x".to_owned(),
            span: Span::new(literal_field_x, literal_field_x + "x".len()),
        })
    );
    assert_eq!(
        analysis.rename_at(literal_field_x, "coord_x"),
        Ok(Some(ql_analysis::RenameResult {
            kind: SymbolKind::Field,
            old_name: "x".to_owned(),
            new_name: "coord_x".to_owned(),
            edits: vec![
                ql_analysis::RenameEdit {
                    span: nth_span(source, "x", 1),
                    replacement: "coord_x".to_owned(),
                },
                ql_analysis::RenameEdit {
                    span: nth_span(source, "x", 2),
                    replacement: "coord_x".to_owned(),
                },
                ql_analysis::RenameEdit {
                    span: nth_span(source, "x", 3),
                    replacement: "coord_x".to_owned(),
                },
                ql_analysis::RenameEdit {
                    span: nth_span(source, "x", 4),
                    replacement: "coord_x".to_owned(),
                },
            ],
        }))
    );
}

#[test]
fn shorthand_struct_field_tokens_stay_on_local_symbols() {
    let source = r#"
struct Point {
    x: Int,
}

fn read(value: Int) -> Int {
    let x = value
    let built = Point { x }
    return x
}
"#;

    let analysis = analyzed(source);
    let shorthand_x = source
        .find("{ x }")
        .map(|offset| offset + 2)
        .expect("shorthand struct literal field should exist");
    let hover = analysis
        .hover_at(shorthand_x)
        .expect("shorthand token should still resolve");

    assert_eq!(hover.kind, SymbolKind::Local);
    assert_eq!(hover.name, "x");
    assert_eq!(hover.definition_span, Some(nth_span(source, "x", 2)));
}

#[test]
fn field_rename_expands_shorthand_struct_sites() {
    let source = r#"
struct Point {
    x: Int,
}

fn read(point: Point, value: Int) -> Int {
    let x = value
    let built = Point { x }
    match point {
        Point { x } => x,
    }
    return point.x
}
"#;

    let analysis = analyzed(source);
    let literal_shorthand = source
        .find("{ x }")
        .map(|offset| offset + 2)
        .expect("shorthand struct literal field should exist");
    let pattern_shorthand = source
        .rfind("{ x }")
        .map(|offset| offset + 2)
        .expect("shorthand struct pattern field should exist");
    let member_use = source
        .rfind(".x")
        .map(|offset| offset + 1)
        .expect("field member use should exist");

    assert_eq!(
        analysis.rename_at(member_use, "coord_x"),
        Ok(Some(ql_analysis::RenameResult {
            kind: SymbolKind::Field,
            old_name: "x".to_owned(),
            new_name: "coord_x".to_owned(),
            edits: vec![
                ql_analysis::RenameEdit {
                    span: nth_span(source, "x", 1),
                    replacement: "coord_x".to_owned(),
                },
                ql_analysis::RenameEdit {
                    span: Span::new(literal_shorthand, literal_shorthand + "x".len()),
                    replacement: "coord_x: x".to_owned(),
                },
                ql_analysis::RenameEdit {
                    span: Span::new(pattern_shorthand, pattern_shorthand + "x".len()),
                    replacement: "coord_x: x".to_owned(),
                },
                ql_analysis::RenameEdit {
                    span: Span::new(member_use, member_use + "x".len()),
                    replacement: "coord_x".to_owned(),
                },
            ],
        }))
    );
}

#[test]
fn local_rename_preserves_shorthand_struct_literal_sites() {
    let source = r#"
struct Point {
    x: Int,
}

fn read(value: Int) -> Int {
    let x = value
    let built = Point { x }
    return x
}
"#;

    let analysis = analyzed(source);
    let literal_shorthand = source
        .find("{ x }")
        .map(|offset| offset + 2)
        .expect("shorthand struct literal field should exist");

    assert_eq!(
        analysis.prepare_rename_at(literal_shorthand),
        Some(ql_analysis::RenameTarget {
            kind: SymbolKind::Local,
            name: "x".to_owned(),
            span: Span::new(literal_shorthand, literal_shorthand + "x".len()),
        })
    );
    assert_eq!(
        analysis.rename_at(literal_shorthand, "coord_x"),
        Ok(Some(ql_analysis::RenameResult {
            kind: SymbolKind::Local,
            old_name: "x".to_owned(),
            new_name: "coord_x".to_owned(),
            edits: vec![
                ql_analysis::RenameEdit {
                    span: nth_span(source, "x", 2),
                    replacement: "coord_x".to_owned(),
                },
                ql_analysis::RenameEdit {
                    span: Span::new(literal_shorthand, literal_shorthand + "x".len()),
                    replacement: "x: coord_x".to_owned(),
                },
                ql_analysis::RenameEdit {
                    span: nth_span(source, "x", 4),
                    replacement: "coord_x".to_owned(),
                },
            ],
        }))
    );
}

#[test]
fn local_rename_preserves_shorthand_struct_pattern_sites() {
    let source = r#"
struct Point {
    x: Int,
}

fn read(point: Point) -> Int {
    return match point {
        Point { x } => x,
    }
}
"#;

    let analysis = analyzed(source);
    let pattern_shorthand = source
        .find("{ x }")
        .map(|offset| offset + 2)
        .expect("shorthand struct pattern field should exist");

    assert_eq!(
        analysis.prepare_rename_at(pattern_shorthand),
        Some(ql_analysis::RenameTarget {
            kind: SymbolKind::Local,
            name: "x".to_owned(),
            span: Span::new(pattern_shorthand, pattern_shorthand + "x".len()),
        })
    );
    assert_eq!(
        analysis.rename_at(pattern_shorthand, "coord_x"),
        Ok(Some(ql_analysis::RenameResult {
            kind: SymbolKind::Local,
            old_name: "x".to_owned(),
            new_name: "coord_x".to_owned(),
            edits: vec![
                ql_analysis::RenameEdit {
                    span: Span::new(pattern_shorthand, pattern_shorthand + "x".len()),
                    replacement: "x: coord_x".to_owned(),
                },
                ql_analysis::RenameEdit {
                    span: nth_span(source, "x", 3),
                    replacement: "coord_x".to_owned(),
                },
            ],
        }))
    );
}

#[test]
fn parameter_rename_preserves_escaped_shorthand_struct_literal_sites() {
    let source = r#"
struct Point {
    `type`: String,
}

fn read(`type`: String) -> String {
    let built = Point { `type` }
    return `type`
}
"#;

    let analysis = analyzed(source);
    let literal_shorthand = source
        .find("{ `type` }")
        .map(|offset| offset + 2)
        .expect("escaped shorthand struct literal field should exist");

    assert_eq!(
        analysis.prepare_rename_at(literal_shorthand),
        Some(ql_analysis::RenameTarget {
            kind: SymbolKind::Parameter,
            name: "type".to_owned(),
            span: Span::new(literal_shorthand, literal_shorthand + "`type`".len()),
        })
    );
    assert_eq!(
        analysis.rename_at(literal_shorthand, "`match`"),
        Ok(Some(ql_analysis::RenameResult {
            kind: SymbolKind::Parameter,
            old_name: "type".to_owned(),
            new_name: "`match`".to_owned(),
            edits: vec![
                ql_analysis::RenameEdit {
                    span: nth_span(source, "`type`", 2),
                    replacement: "`match`".to_owned(),
                },
                ql_analysis::RenameEdit {
                    span: Span::new(literal_shorthand, literal_shorthand + "`type`".len(),),
                    replacement: "`type`: `match`".to_owned(),
                },
                ql_analysis::RenameEdit {
                    span: nth_span(source, "`type`", 4),
                    replacement: "`match`".to_owned(),
                },
            ],
        }))
    );
}

#[test]
fn import_alias_rename_preserves_shorthand_struct_literal_sites() {
    let source = r#"
use source_value as source

struct Point {
    source: Int,
}

const source_value: Int = 1

fn read() -> Int {
    let built = Point { source }
    return source
}
"#;

    let analysis = analyzed(source);
    let literal_shorthand = source
        .find("{ source }")
        .map(|offset| offset + 2)
        .expect("import-alias shorthand struct literal field should exist");
    let return_source = source
        .rfind("return source")
        .map(|offset| offset + "return ".len())
        .expect("return source use should exist");

    assert_eq!(
        analysis.prepare_rename_at(literal_shorthand),
        Some(ql_analysis::RenameTarget {
            kind: SymbolKind::Import,
            name: "source".to_owned(),
            span: Span::new(literal_shorthand, literal_shorthand + "source".len()),
        })
    );
    assert_eq!(
        analysis.rename_at(literal_shorthand, "feed"),
        Ok(Some(ql_analysis::RenameResult {
            kind: SymbolKind::Import,
            old_name: "source".to_owned(),
            new_name: "feed".to_owned(),
            edits: vec![
                ql_analysis::RenameEdit {
                    span: alias_span(source, "source"),
                    replacement: "feed".to_owned(),
                },
                ql_analysis::RenameEdit {
                    span: Span::new(literal_shorthand, literal_shorthand + "source".len(),),
                    replacement: "source: feed".to_owned(),
                },
                ql_analysis::RenameEdit {
                    span: Span::new(return_source, return_source + "source".len()),
                    replacement: "feed".to_owned(),
                },
            ],
        }))
    );
}

#[test]
fn function_rename_preserves_shorthand_struct_literal_sites() {
    let source = r#"
struct Ops {
    add_one: (Int) -> Int,
}

fn add_one(value: Int) -> Int {
    return value + 1
}

fn read() -> Int {
    let built = Ops { add_one }
    return add_one(1)
}
"#;

    let analysis = analyzed(source);
    let literal_shorthand = source
        .find("{ add_one }")
        .map(|offset| offset + 2)
        .expect("function shorthand struct literal field should exist");
    let function_name = source
        .find("fn add_one")
        .map(|offset| offset + "fn ".len())
        .expect("function definition should exist");
    let call_site = source
        .rfind("add_one(1)")
        .expect("function call should exist");

    assert_eq!(
        analysis.prepare_rename_at(literal_shorthand),
        Some(ql_analysis::RenameTarget {
            kind: SymbolKind::Function,
            name: "add_one".to_owned(),
            span: Span::new(literal_shorthand, literal_shorthand + "add_one".len()),
        })
    );
    assert_eq!(
        analysis.rename_at(literal_shorthand, "inc"),
        Ok(Some(ql_analysis::RenameResult {
            kind: SymbolKind::Function,
            old_name: "add_one".to_owned(),
            new_name: "inc".to_owned(),
            edits: vec![
                ql_analysis::RenameEdit {
                    span: Span::new(function_name, function_name + "add_one".len()),
                    replacement: "inc".to_owned(),
                },
                ql_analysis::RenameEdit {
                    span: nth_span(source, "add_one", 3),
                    replacement: "add_one: inc".to_owned(),
                },
                ql_analysis::RenameEdit {
                    span: Span::new(call_site, call_site + "add_one".len()),
                    replacement: "inc".to_owned(),
                },
            ],
        }))
    );
}

#[test]
fn const_rename_preserves_shorthand_struct_literal_sites() {
    let source = r#"
struct Limits {
    max: Int,
}

const max: Int = 10

fn read() -> Int {
    let built = Limits { max }
    return max
}
"#;

    let analysis = analyzed(source);
    let literal_shorthand = source
        .find("{ max }")
        .map(|offset| offset + 2)
        .expect("const shorthand struct literal field should exist");
    let const_name = source
        .find("const max")
        .map(|offset| offset + "const ".len())
        .expect("const definition should exist");
    let return_use = source
        .rfind("return max")
        .map(|offset| offset + "return ".len())
        .expect("const return use should exist");

    assert_eq!(
        analysis.prepare_rename_at(literal_shorthand),
        Some(ql_analysis::RenameTarget {
            kind: SymbolKind::Const,
            name: "max".to_owned(),
            span: Span::new(literal_shorthand, literal_shorthand + "max".len()),
        })
    );
    assert_eq!(
        analysis.rename_at(literal_shorthand, "upper_bound"),
        Ok(Some(ql_analysis::RenameResult {
            kind: SymbolKind::Const,
            old_name: "max".to_owned(),
            new_name: "upper_bound".to_owned(),
            edits: vec![
                ql_analysis::RenameEdit {
                    span: Span::new(const_name, const_name + "max".len()),
                    replacement: "upper_bound".to_owned(),
                },
                ql_analysis::RenameEdit {
                    span: Span::new(literal_shorthand, literal_shorthand + "max".len()),
                    replacement: "max: upper_bound".to_owned(),
                },
                ql_analysis::RenameEdit {
                    span: Span::new(return_use, return_use + "max".len()),
                    replacement: "upper_bound".to_owned(),
                },
            ],
        }))
    );
}

#[test]
fn static_rename_preserves_shorthand_struct_literal_sites() {
    let source = r#"
struct Limits {
    current: Int,
}

static current: Int = 10

fn read() -> Int {
    let built = Limits { current }
    return current
}
"#;

    let analysis = analyzed(source);
    let literal_shorthand = source
        .find("{ current }")
        .map(|offset| offset + 2)
        .expect("static shorthand struct literal field should exist");
    let static_name = source
        .find("static current")
        .map(|offset| offset + "static ".len())
        .expect("static definition should exist");
    let return_use = source
        .rfind("return current")
        .map(|offset| offset + "return ".len())
        .expect("static return use should exist");

    assert_eq!(
        analysis.prepare_rename_at(literal_shorthand),
        Some(ql_analysis::RenameTarget {
            kind: SymbolKind::Static,
            name: "current".to_owned(),
            span: Span::new(literal_shorthand, literal_shorthand + "current".len()),
        })
    );
    assert_eq!(
        analysis.rename_at(literal_shorthand, "current_value"),
        Ok(Some(ql_analysis::RenameResult {
            kind: SymbolKind::Static,
            old_name: "current".to_owned(),
            new_name: "current_value".to_owned(),
            edits: vec![
                ql_analysis::RenameEdit {
                    span: Span::new(static_name, static_name + "current".len()),
                    replacement: "current_value".to_owned(),
                },
                ql_analysis::RenameEdit {
                    span: Span::new(literal_shorthand, literal_shorthand + "current".len(),),
                    replacement: "current: current_value".to_owned(),
                },
                ql_analysis::RenameEdit {
                    span: Span::new(return_use, return_use + "current".len()),
                    replacement: "current_value".to_owned(),
                },
            ],
        }))
    );
}

#[test]
fn explicit_struct_field_labels_follow_same_file_import_alias_roots() {
    let source = r#"
use Point as P

struct Point {
    x: Int,
    y: Int,
}

fn read(point: Point, value: Int) -> Int {
    let built = P { x: value, y: 1 }
    match point {
        P { x: alias, y: 2 } => alias,
    }
    return point.x
}
"#;

    let analysis = analyzed(source);
    let literal_field_x = source
        .find("{ x: value")
        .map(|offset| offset + 2)
        .expect("explicit struct literal field through import alias should exist");
    let pattern_field_x = source
        .find("{ x: alias")
        .map(|offset| offset + 2)
        .expect("explicit struct pattern field through import alias should exist");

    let hover = analysis
        .hover_at(literal_field_x)
        .expect("explicit field label through import alias should hover");
    assert_eq!(hover.kind, SymbolKind::Field);
    assert_eq!(hover.detail, "field x: Int");
    assert_eq!(hover.ty.as_deref(), Some("Int"));
    assert_eq!(hover.definition_span, Some(nth_span(source, "x", 1)));

    assert_eq!(
        analysis.definition_at(pattern_field_x),
        Some(ql_analysis::DefinitionTarget {
            kind: SymbolKind::Field,
            name: "x".to_owned(),
            span: nth_span(source, "x", 1),
        })
    );
    assert_eq!(
        analysis.references_at(literal_field_x),
        Some(vec![
            ql_analysis::ReferenceTarget {
                kind: SymbolKind::Field,
                name: "x".to_owned(),
                span: nth_span(source, "x", 1),
                is_definition: true,
            },
            ql_analysis::ReferenceTarget {
                kind: SymbolKind::Field,
                name: "x".to_owned(),
                span: nth_span(source, "x", 2),
                is_definition: false,
            },
            ql_analysis::ReferenceTarget {
                kind: SymbolKind::Field,
                name: "x".to_owned(),
                span: nth_span(source, "x", 3),
                is_definition: false,
            },
            ql_analysis::ReferenceTarget {
                kind: SymbolKind::Field,
                name: "x".to_owned(),
                span: nth_span(source, "x", 4),
                is_definition: false,
            },
        ])
    );
    assert_eq!(
        analysis.prepare_rename_at(literal_field_x),
        Some(ql_analysis::RenameTarget {
            kind: SymbolKind::Field,
            name: "x".to_owned(),
            span: Span::new(literal_field_x, literal_field_x + "x".len()),
        })
    );
    assert_eq!(
        analysis.rename_at(literal_field_x, "coord_x"),
        Ok(Some(ql_analysis::RenameResult {
            kind: SymbolKind::Field,
            old_name: "x".to_owned(),
            new_name: "coord_x".to_owned(),
            edits: vec![
                ql_analysis::RenameEdit {
                    span: nth_span(source, "x", 1),
                    replacement: "coord_x".to_owned(),
                },
                ql_analysis::RenameEdit {
                    span: nth_span(source, "x", 2),
                    replacement: "coord_x".to_owned(),
                },
                ql_analysis::RenameEdit {
                    span: nth_span(source, "x", 3),
                    replacement: "coord_x".to_owned(),
                },
                ql_analysis::RenameEdit {
                    span: nth_span(source, "x", 4),
                    replacement: "coord_x".to_owned(),
                },
            ],
        }))
    );
}

#[test]
fn field_rename_expands_shorthand_struct_sites_through_import_alias_paths() {
    let source = r#"
use Point as P

struct Point {
    x: Int,
}

fn read(point: Point, value: Int) -> Int {
    let x = value
    let built = P { x }
    match point {
        P { x } => x,
    }
    return point.x
}
"#;

    let analysis = analyzed(source);
    let literal_shorthand = source
        .find("{ x }")
        .map(|offset| offset + 2)
        .expect("shorthand struct literal field through import alias should exist");
    let pattern_shorthand = source
        .rfind("{ x }")
        .map(|offset| offset + 2)
        .expect("shorthand struct pattern field through import alias should exist");
    let member_use = source
        .rfind(".x")
        .map(|offset| offset + 1)
        .expect("field member use should exist");

    assert_eq!(
        analysis.rename_at(member_use, "coord_x"),
        Ok(Some(ql_analysis::RenameResult {
            kind: SymbolKind::Field,
            old_name: "x".to_owned(),
            new_name: "coord_x".to_owned(),
            edits: vec![
                ql_analysis::RenameEdit {
                    span: nth_span(source, "x", 1),
                    replacement: "coord_x".to_owned(),
                },
                ql_analysis::RenameEdit {
                    span: Span::new(literal_shorthand, literal_shorthand + "x".len()),
                    replacement: "coord_x: x".to_owned(),
                },
                ql_analysis::RenameEdit {
                    span: Span::new(pattern_shorthand, pattern_shorthand + "x".len()),
                    replacement: "coord_x: x".to_owned(),
                },
                ql_analysis::RenameEdit {
                    span: Span::new(member_use, member_use + "x".len()),
                    replacement: "coord_x".to_owned(),
                },
            ],
        }))
    );
}

#[test]
fn local_rename_preserves_shorthand_struct_sites_through_import_alias_paths() {
    let source = r#"
use Point as P

struct Point {
    x: Int,
}

fn read(value: Int) -> Int {
    let x = value
    let built = P { x }
    return x
}
"#;

    let analysis = analyzed(source);
    let literal_shorthand = source
        .find("{ x }")
        .map(|offset| offset + 2)
        .expect("shorthand struct literal field through import alias should exist");
    let return_x = source
        .rfind("return x")
        .map(|offset| offset + "return ".len())
        .expect("return x should exist");

    assert_eq!(
        analysis.prepare_rename_at(literal_shorthand),
        Some(ql_analysis::RenameTarget {
            kind: SymbolKind::Local,
            name: "x".to_owned(),
            span: Span::new(literal_shorthand, literal_shorthand + "x".len()),
        })
    );
    assert_eq!(
        analysis.rename_at(literal_shorthand, "coord_x"),
        Ok(Some(ql_analysis::RenameResult {
            kind: SymbolKind::Local,
            old_name: "x".to_owned(),
            new_name: "coord_x".to_owned(),
            edits: vec![
                ql_analysis::RenameEdit {
                    span: nth_span(source, "x", 2),
                    replacement: "coord_x".to_owned(),
                },
                ql_analysis::RenameEdit {
                    span: Span::new(literal_shorthand, literal_shorthand + "x".len()),
                    replacement: "x: coord_x".to_owned(),
                },
                ql_analysis::RenameEdit {
                    span: Span::new(return_x, return_x + "x".len()),
                    replacement: "coord_x".to_owned(),
                },
            ],
        }))
    );
}

#[test]
fn local_rename_preserves_shorthand_struct_pattern_sites_through_import_alias_paths() {
    let source = r#"
use Point as P

struct Point {
    x: Int,
}

fn read(point: Point) -> Int {
    return match point {
        P { x } => x,
    }
}
"#;

    let analysis = analyzed(source);
    let pattern_shorthand = source
        .find("{ x }")
        .map(|offset| offset + 2)
        .expect("shorthand struct pattern field through import alias should exist");
    let arm_use = source
        .rfind("=> x")
        .map(|offset| offset + "=> ".len())
        .expect("match arm use should exist");

    assert_eq!(
        analysis.prepare_rename_at(pattern_shorthand),
        Some(ql_analysis::RenameTarget {
            kind: SymbolKind::Local,
            name: "x".to_owned(),
            span: Span::new(pattern_shorthand, pattern_shorthand + "x".len()),
        })
    );
    assert_eq!(
        analysis.rename_at(pattern_shorthand, "coord_x"),
        Ok(Some(ql_analysis::RenameResult {
            kind: SymbolKind::Local,
            old_name: "x".to_owned(),
            new_name: "coord_x".to_owned(),
            edits: vec![
                ql_analysis::RenameEdit {
                    span: Span::new(pattern_shorthand, pattern_shorthand + "x".len()),
                    replacement: "x: coord_x".to_owned(),
                },
                ql_analysis::RenameEdit {
                    span: Span::new(arm_use, arm_use + "x".len()),
                    replacement: "coord_x".to_owned(),
                },
            ],
        }))
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
fn direct_symbol_queries_follow_same_file_surface_aggregate() {
    struct DirectCase<'a> {
        kind: SymbolKind,
        name: &'a str,
        use_occurrence: usize,
        detail: &'a str,
        reference_occurrences: &'a [usize],
    }

    let source = r#"
enum Command {
    Retry(Int),
    Config { retries: Int },
}

struct Point {
    x: Int,
    y: Int,
}

fn build(flag: Bool) -> Command {
    if flag {
        return Command.Retry(1)
    }
    return Command.Config { retries: 2 }
}

fn project(point: Point, value: Int) -> Int {
    let built = Point { x: value, y: 1 }
    match point {
        Point { x: alias, y: 2 } => alias,
    }
    return 0
}
"#;

    let analysis = analyzed(source);
    let cases = [
        DirectCase {
            kind: SymbolKind::Variant,
            name: "Retry",
            use_occurrence: 2,
            detail: "variant Command.Retry(Int)",
            reference_occurrences: &[1, 2],
        },
        DirectCase {
            kind: SymbolKind::Variant,
            name: "Config",
            use_occurrence: 2,
            detail: "variant Command.Config { retries: Int }",
            reference_occurrences: &[1, 2],
        },
        DirectCase {
            kind: SymbolKind::Field,
            name: "x",
            use_occurrence: 2,
            detail: "field x: Int",
            reference_occurrences: &[1, 2, 3],
        },
    ];

    for case in cases {
        let use_offset = nth_offset(source, case.name, case.use_occurrence);
        let hover = analysis
            .hover_at(use_offset)
            .expect("direct symbol hover should exist");

        assert_eq!(hover.kind, case.kind, "{}", case.name);
        assert_eq!(hover.detail, case.detail, "{}", case.name);
        assert_eq!(
            hover.definition_span,
            Some(nth_span(source, case.name, 1)),
            "{}",
            case.name
        );
        assert_eq!(
            analysis.definition_at(use_offset),
            Some(ql_analysis::DefinitionTarget {
                kind: case.kind,
                name: case.name.to_owned(),
                span: nth_span(source, case.name, 1),
            }),
            "{}",
            case.name
        );
        assert_eq!(
            analysis.references_at(use_offset),
            Some(
                case.reference_occurrences
                    .iter()
                    .map(|occurrence| ql_analysis::ReferenceTarget {
                        kind: case.kind,
                        name: case.name.to_owned(),
                        span: nth_span(source, case.name, *occurrence),
                        is_definition: *occurrence == 1,
                    })
                    .collect::<Vec<_>>()
            ),
            "{}",
            case.name
        );
    }
}

#[test]
fn variant_queries_follow_same_file_import_alias_roots() {
    let source = r#"
use Command as Cmd

enum Command {
    Retry(Int),
    Config { retries: Int },
}

fn build(flag: Bool) -> Command {
    if flag {
        return Cmd.Retry(1)
    }
    return Cmd.Config { retries: 2 }
}

fn read(command: Command) -> Int {
    match command {
        Cmd.Retry(times) => times,
        Cmd.Config { retries } => retries,
    }
}
"#;

    let analysis = analyzed(source);
    let retry_use = source
        .find("Cmd.Retry(")
        .map(|offset| offset + "Cmd.".len())
        .expect("retry constructor through import alias should exist");
    let config_literal_use = source
        .find("Cmd.Config {")
        .map(|offset| offset + "Cmd.".len())
        .expect("config struct literal through import alias should exist");
    let config_pattern_use = source
        .rfind("Cmd.Config {")
        .map(|offset| offset + "Cmd.".len())
        .expect("config pattern through import alias should exist");

    let retry_hover = analysis
        .hover_at(retry_use)
        .expect("retry variant hover through import alias should exist");
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
        .expect("config variant hover through import alias should exist");
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
fn rename_queries_follow_function_const_and_static_symbols() {
    let source = r#"
const LIMIT: Int = 10

static CURRENT: Int = LIMIT

fn compute(value: Int) -> Int {
    return value + LIMIT
}

fn read() -> Int {
    return compute(CURRENT) + LIMIT
}
"#;

    let analysis = analyzed(source);
    let function_use = nth_offset(source, "compute", 2);
    let const_use = nth_offset(source, "LIMIT", 4);
    let static_use = nth_offset(source, "CURRENT", 2);

    assert_eq!(
        analysis.prepare_rename_at(function_use),
        Some(ql_analysis::RenameTarget {
            kind: SymbolKind::Function,
            name: "compute".to_owned(),
            span: nth_span(source, "compute", 2),
        })
    );
    assert_eq!(
        analysis.rename_at(function_use, "measure"),
        Ok(Some(ql_analysis::RenameResult {
            kind: SymbolKind::Function,
            old_name: "compute".to_owned(),
            new_name: "measure".to_owned(),
            edits: vec![
                ql_analysis::RenameEdit {
                    span: nth_span(source, "compute", 1),
                    replacement: "measure".to_owned(),
                },
                ql_analysis::RenameEdit {
                    span: nth_span(source, "compute", 2),
                    replacement: "measure".to_owned(),
                },
            ],
        }))
    );

    assert_eq!(
        analysis.prepare_rename_at(const_use),
        Some(ql_analysis::RenameTarget {
            kind: SymbolKind::Const,
            name: "LIMIT".to_owned(),
            span: nth_span(source, "LIMIT", 4),
        })
    );
    assert_eq!(
        analysis.rename_at(const_use, "MAX_LIMIT"),
        Ok(Some(ql_analysis::RenameResult {
            kind: SymbolKind::Const,
            old_name: "LIMIT".to_owned(),
            new_name: "MAX_LIMIT".to_owned(),
            edits: vec![
                ql_analysis::RenameEdit {
                    span: nth_span(source, "LIMIT", 1),
                    replacement: "MAX_LIMIT".to_owned(),
                },
                ql_analysis::RenameEdit {
                    span: nth_span(source, "LIMIT", 2),
                    replacement: "MAX_LIMIT".to_owned(),
                },
                ql_analysis::RenameEdit {
                    span: nth_span(source, "LIMIT", 3),
                    replacement: "MAX_LIMIT".to_owned(),
                },
                ql_analysis::RenameEdit {
                    span: nth_span(source, "LIMIT", 4),
                    replacement: "MAX_LIMIT".to_owned(),
                },
            ],
        }))
    );

    assert_eq!(
        analysis.prepare_rename_at(static_use),
        Some(ql_analysis::RenameTarget {
            kind: SymbolKind::Static,
            name: "CURRENT".to_owned(),
            span: nth_span(source, "CURRENT", 2),
        })
    );
    assert_eq!(
        analysis.rename_at(static_use, "LATEST"),
        Ok(Some(ql_analysis::RenameResult {
            kind: SymbolKind::Static,
            old_name: "CURRENT".to_owned(),
            new_name: "LATEST".to_owned(),
            edits: vec![
                ql_analysis::RenameEdit {
                    span: nth_span(source, "CURRENT", 1),
                    replacement: "LATEST".to_owned(),
                },
                ql_analysis::RenameEdit {
                    span: nth_span(source, "CURRENT", 2),
                    replacement: "LATEST".to_owned(),
                },
            ],
        }))
    );
}

#[test]
fn rename_queries_follow_lexical_supported_and_closed_symbols() {
    let source = r#"
fn id[T](param: T) -> T {
    let local_value = param
    return local_value
}

struct Counter {
    value: String,
}

impl Counter {
    fn read(self, input: String) -> String {
        let alias = input
        return self.value
    }
}
"#;

    let analysis = analyzed(source);
    let generic_use = nth_offset(source, "T", 3);
    let parameter_use = nth_offset(source, "param", 2);
    let local_use = nth_offset(source, "local_value", 2);
    let self_use = source.find("self.value").expect("self use should exist");
    let builtin_use = nth_offset(source, "String", 2);

    assert_eq!(
        analysis.prepare_rename_at(generic_use),
        Some(ql_analysis::RenameTarget {
            kind: SymbolKind::Generic,
            name: "T".to_owned(),
            span: nth_span(source, "T", 3),
        })
    );
    assert_eq!(
        analysis.rename_at(generic_use, "Value"),
        Ok(Some(ql_analysis::RenameResult {
            kind: SymbolKind::Generic,
            old_name: "T".to_owned(),
            new_name: "Value".to_owned(),
            edits: vec![
                ql_analysis::RenameEdit {
                    span: nth_span(source, "T", 1),
                    replacement: "Value".to_owned(),
                },
                ql_analysis::RenameEdit {
                    span: nth_span(source, "T", 2),
                    replacement: "Value".to_owned(),
                },
                ql_analysis::RenameEdit {
                    span: nth_span(source, "T", 3),
                    replacement: "Value".to_owned(),
                },
            ],
        }))
    );

    assert_eq!(
        analysis.prepare_rename_at(parameter_use),
        Some(ql_analysis::RenameTarget {
            kind: SymbolKind::Parameter,
            name: "param".to_owned(),
            span: nth_span(source, "param", 2),
        })
    );
    assert_eq!(
        analysis.rename_at(parameter_use, "input_value"),
        Ok(Some(ql_analysis::RenameResult {
            kind: SymbolKind::Parameter,
            old_name: "param".to_owned(),
            new_name: "input_value".to_owned(),
            edits: vec![
                ql_analysis::RenameEdit {
                    span: nth_span(source, "param", 1),
                    replacement: "input_value".to_owned(),
                },
                ql_analysis::RenameEdit {
                    span: nth_span(source, "param", 2),
                    replacement: "input_value".to_owned(),
                },
            ],
        }))
    );

    assert_eq!(
        analysis.prepare_rename_at(local_use),
        Some(ql_analysis::RenameTarget {
            kind: SymbolKind::Local,
            name: "local_value".to_owned(),
            span: nth_span(source, "local_value", 2),
        })
    );
    assert_eq!(
        analysis.rename_at(local_use, "result_value"),
        Ok(Some(ql_analysis::RenameResult {
            kind: SymbolKind::Local,
            old_name: "local_value".to_owned(),
            new_name: "result_value".to_owned(),
            edits: vec![
                ql_analysis::RenameEdit {
                    span: nth_span(source, "local_value", 1),
                    replacement: "result_value".to_owned(),
                },
                ql_analysis::RenameEdit {
                    span: nth_span(source, "local_value", 2),
                    replacement: "result_value".to_owned(),
                },
            ],
        }))
    );

    assert_eq!(analysis.prepare_rename_at(self_use), None);
    assert_eq!(analysis.rename_at(self_use, "owner"), Ok(None));

    assert_eq!(analysis.prepare_rename_at(builtin_use), None);
    assert_eq!(analysis.rename_at(builtin_use, "Text"), Ok(None));
}

#[test]
fn rename_queries_follow_supported_type_namespace_item_symbols() {
    let source = r#"
type UserId = Int

struct User {
    id: UserId,
}

enum Status {
    Active,
}

trait Named {
    fn id(self) -> UserId
}

impl Named for User {
    fn id(self) -> UserId {
        return self.id
    }
}

fn active(user: User, current: UserId) -> Status {
    let next = current
    return Status.Active
}
"#;

    let analysis = analyzed(source);
    let type_alias_use = nth_offset(source, "UserId", 2);
    let struct_def = source
        .find("struct User")
        .map(|offset| offset + "struct ".len())
        .expect("struct definition should exist");
    let struct_use = source
        .find("for User")
        .map(|offset| offset + "for ".len())
        .expect("struct use in impl header should exist");
    let struct_param_use = source
        .find("user: User")
        .map(|offset| offset + "user: ".len())
        .expect("struct use in function parameter should exist");
    let enum_use = source
        .find("-> Status")
        .map(|offset| offset + 3)
        .expect("enum use in return type should exist");
    let trait_use = source
        .find("impl Named")
        .map(|offset| offset + "impl ".len())
        .expect("trait use in impl header should exist");

    assert_eq!(
        analysis.prepare_rename_at(type_alias_use),
        Some(ql_analysis::RenameTarget {
            kind: SymbolKind::TypeAlias,
            name: "UserId".to_owned(),
            span: nth_span(source, "UserId", 2),
        })
    );
    assert_eq!(
        analysis.rename_at(type_alias_use, "AccountId"),
        Ok(Some(ql_analysis::RenameResult {
            kind: SymbolKind::TypeAlias,
            old_name: "UserId".to_owned(),
            new_name: "AccountId".to_owned(),
            edits: vec![
                ql_analysis::RenameEdit {
                    span: nth_span(source, "UserId", 1),
                    replacement: "AccountId".to_owned(),
                },
                ql_analysis::RenameEdit {
                    span: nth_span(source, "UserId", 2),
                    replacement: "AccountId".to_owned(),
                },
                ql_analysis::RenameEdit {
                    span: nth_span(source, "UserId", 3),
                    replacement: "AccountId".to_owned(),
                },
                ql_analysis::RenameEdit {
                    span: nth_span(source, "UserId", 4),
                    replacement: "AccountId".to_owned(),
                },
                ql_analysis::RenameEdit {
                    span: nth_span(source, "UserId", 5),
                    replacement: "AccountId".to_owned(),
                },
            ],
        }))
    );

    assert_eq!(
        analysis.prepare_rename_at(struct_use),
        Some(ql_analysis::RenameTarget {
            kind: SymbolKind::Struct,
            name: "User".to_owned(),
            span: Span::new(struct_use, struct_use + "User".len()),
        })
    );
    assert_eq!(
        analysis.rename_at(struct_use, "Member"),
        Ok(Some(ql_analysis::RenameResult {
            kind: SymbolKind::Struct,
            old_name: "User".to_owned(),
            new_name: "Member".to_owned(),
            edits: vec![
                ql_analysis::RenameEdit {
                    span: Span::new(struct_def, struct_def + "User".len()),
                    replacement: "Member".to_owned(),
                },
                ql_analysis::RenameEdit {
                    span: Span::new(struct_use, struct_use + "User".len()),
                    replacement: "Member".to_owned(),
                },
                ql_analysis::RenameEdit {
                    span: Span::new(struct_param_use, struct_param_use + "User".len()),
                    replacement: "Member".to_owned(),
                },
            ],
        }))
    );

    assert_eq!(
        analysis.prepare_rename_at(enum_use),
        Some(ql_analysis::RenameTarget {
            kind: SymbolKind::Enum,
            name: "Status".to_owned(),
            span: Span::new(enum_use, enum_use + "Status".len()),
        })
    );
    assert_eq!(
        analysis.rename_at(enum_use, "Phase"),
        Ok(Some(ql_analysis::RenameResult {
            kind: SymbolKind::Enum,
            old_name: "Status".to_owned(),
            new_name: "Phase".to_owned(),
            edits: vec![
                ql_analysis::RenameEdit {
                    span: nth_span(source, "Status", 1),
                    replacement: "Phase".to_owned(),
                },
                ql_analysis::RenameEdit {
                    span: nth_span(source, "Status", 2),
                    replacement: "Phase".to_owned(),
                },
                ql_analysis::RenameEdit {
                    span: nth_span(source, "Status", 3),
                    replacement: "Phase".to_owned(),
                },
            ],
        }))
    );

    assert_eq!(
        analysis.prepare_rename_at(trait_use),
        Some(ql_analysis::RenameTarget {
            kind: SymbolKind::Trait,
            name: "Named".to_owned(),
            span: Span::new(trait_use, trait_use + "Named".len()),
        })
    );
    assert_eq!(
        analysis.rename_at(trait_use, "Identified"),
        Ok(Some(ql_analysis::RenameResult {
            kind: SymbolKind::Trait,
            old_name: "Named".to_owned(),
            new_name: "Identified".to_owned(),
            edits: vec![
                ql_analysis::RenameEdit {
                    span: nth_span(source, "Named", 1),
                    replacement: "Identified".to_owned(),
                },
                ql_analysis::RenameEdit {
                    span: nth_span(source, "Named", 2),
                    replacement: "Identified".to_owned(),
                },
            ],
        }))
    );
}

#[test]
fn type_namespace_item_reference_queries_follow_same_file_identity() {
    let source = r#"
type IdAlias = Int

struct Account {
    id: IdAlias,
}

enum Mode {
    Ready,
}

trait Taggable {
    fn mode(self) -> Mode
}

impl Taggable for Account {
    fn mode(self) -> Mode {
        return Mode.Ready
    }
}

fn build(account: Account, value: IdAlias) -> Mode {
    let copy = value
    return account.mode()
}
"#;

    let analysis = analyzed(source);
    let type_alias_use = source
        .find("id: IdAlias")
        .map(|offset| offset + "id: ".len())
        .expect("type alias field use should exist");
    let struct_use = source
        .find("for Account")
        .map(|offset| offset + "for ".len())
        .expect("struct use in impl header should exist");
    let enum_use = source
        .rfind("-> Mode")
        .map(|offset| offset + 3)
        .expect("enum use in function return type should exist");
    let trait_use = source
        .find("impl Taggable")
        .map(|offset| offset + "impl ".len())
        .expect("trait use in impl header should exist");

    assert_eq!(
        analysis.references_at(type_alias_use),
        Some(vec![
            ql_analysis::ReferenceTarget {
                kind: SymbolKind::TypeAlias,
                name: "IdAlias".to_owned(),
                span: nth_span(source, "IdAlias", 1),
                is_definition: true,
            },
            ql_analysis::ReferenceTarget {
                kind: SymbolKind::TypeAlias,
                name: "IdAlias".to_owned(),
                span: nth_span(source, "IdAlias", 2),
                is_definition: false,
            },
            ql_analysis::ReferenceTarget {
                kind: SymbolKind::TypeAlias,
                name: "IdAlias".to_owned(),
                span: nth_span(source, "IdAlias", 3),
                is_definition: false,
            },
        ])
    );

    assert_eq!(
        analysis.references_at(struct_use),
        Some(vec![
            ql_analysis::ReferenceTarget {
                kind: SymbolKind::Struct,
                name: "Account".to_owned(),
                span: nth_span(source, "Account", 1),
                is_definition: true,
            },
            ql_analysis::ReferenceTarget {
                kind: SymbolKind::Struct,
                name: "Account".to_owned(),
                span: nth_span(source, "Account", 2),
                is_definition: false,
            },
            ql_analysis::ReferenceTarget {
                kind: SymbolKind::Struct,
                name: "Account".to_owned(),
                span: nth_span(source, "Account", 3),
                is_definition: false,
            },
        ])
    );

    assert_eq!(
        analysis.references_at(enum_use),
        Some(vec![
            ql_analysis::ReferenceTarget {
                kind: SymbolKind::Enum,
                name: "Mode".to_owned(),
                span: nth_span(source, "Mode", 1),
                is_definition: true,
            },
            ql_analysis::ReferenceTarget {
                kind: SymbolKind::Enum,
                name: "Mode".to_owned(),
                span: nth_span(source, "Mode", 2),
                is_definition: false,
            },
            ql_analysis::ReferenceTarget {
                kind: SymbolKind::Enum,
                name: "Mode".to_owned(),
                span: nth_span(source, "Mode", 3),
                is_definition: false,
            },
            ql_analysis::ReferenceTarget {
                kind: SymbolKind::Enum,
                name: "Mode".to_owned(),
                span: nth_span(source, "Mode", 4),
                is_definition: false,
            },
            ql_analysis::ReferenceTarget {
                kind: SymbolKind::Enum,
                name: "Mode".to_owned(),
                span: nth_span(source, "Mode", 5),
                is_definition: false,
            },
        ])
    );

    assert_eq!(
        analysis.references_at(trait_use),
        Some(vec![
            ql_analysis::ReferenceTarget {
                kind: SymbolKind::Trait,
                name: "Taggable".to_owned(),
                span: nth_span(source, "Taggable", 1),
                is_definition: true,
            },
            ql_analysis::ReferenceTarget {
                kind: SymbolKind::Trait,
                name: "Taggable".to_owned(),
                span: nth_span(source, "Taggable", 2),
                is_definition: false,
            },
        ])
    );
}

#[test]
fn type_namespace_item_queries_follow_same_file_surface_aggregate() {
    struct ItemCase<'a> {
        kind: SymbolKind,
        name: &'a str,
        use_occurrence: usize,
        detail: &'a str,
        reference_occurrences: &'a [usize],
    }

    let source = r#"
type IdAlias = Int

opaque type UserId = Int

struct Account {
    id: UserId,
    alias: IdAlias,
}

enum Mode {
    Ready,
}

trait Taggable {
    fn mode(self) -> Mode
}

impl Taggable for Account {
    fn mode(self) -> Mode {
        return Mode.Ready
    }
}

fn build(account: Account, user_id: UserId, alias: IdAlias) -> Mode {
    let copied_id = user_id
    let copied_alias = alias
    return account.mode()
}
"#;

    let analysis = analyzed(source);
    let cases = [
        ItemCase {
            kind: SymbolKind::TypeAlias,
            name: "IdAlias",
            use_occurrence: 3,
            detail: "type IdAlias = Int",
            reference_occurrences: &[1, 2, 3],
        },
        ItemCase {
            kind: SymbolKind::TypeAlias,
            name: "UserId",
            use_occurrence: 3,
            detail: "opaque type UserId = Int",
            reference_occurrences: &[1, 2, 3],
        },
        ItemCase {
            kind: SymbolKind::Struct,
            name: "Account",
            use_occurrence: 3,
            detail: "struct Account",
            reference_occurrences: &[1, 2, 3],
        },
        ItemCase {
            kind: SymbolKind::Enum,
            name: "Mode",
            use_occurrence: 5,
            detail: "enum Mode",
            reference_occurrences: &[1, 2, 3, 4, 5],
        },
        ItemCase {
            kind: SymbolKind::Trait,
            name: "Taggable",
            use_occurrence: 2,
            detail: "trait Taggable",
            reference_occurrences: &[1, 2],
        },
    ];

    for case in cases {
        let use_offset = nth_offset(source, case.name, case.use_occurrence);
        let hover = analysis
            .hover_at(use_offset)
            .expect("type-namespace item hover should exist");

        assert_eq!(hover.kind, case.kind, "{}", case.name);
        assert_eq!(hover.detail, case.detail, "{}", case.name);
        assert_eq!(
            hover.definition_span,
            Some(nth_span(source, case.name, 1)),
            "{}",
            case.name
        );
        assert_eq!(
            analysis.definition_at(use_offset),
            Some(ql_analysis::DefinitionTarget {
                kind: case.kind,
                name: case.name.to_owned(),
                span: nth_span(source, case.name, 1),
            }),
            "{}",
            case.name
        );
        assert_eq!(
            analysis.references_at(use_offset),
            Some(
                case.reference_occurrences
                    .iter()
                    .map(|occurrence| ql_analysis::ReferenceTarget {
                        kind: case.kind,
                        name: case.name.to_owned(),
                        span: nth_span(source, case.name, *occurrence),
                        is_definition: *occurrence == 1,
                    })
                    .collect::<Vec<_>>()
            ),
            "{}",
            case.name
        );
    }
}

#[test]
fn rename_queries_follow_variant_symbols_through_import_alias_paths() {
    let source = r#"
use Command as Cmd

enum Command {
    Retry(Int),
    Stop,
}

fn build(flag: Bool) -> Command {
    if flag {
        return Cmd.Retry(1)
    }
    return Cmd.Stop
}

fn read(command: Command) -> Int {
    match command {
        Cmd.Retry(times) => times,
        Cmd.Stop => 0,
    }
}
"#;

    let analysis = analyzed(source);
    let retry_use = source
        .find("Cmd.Retry(")
        .map(|offset| offset + "Cmd.".len())
        .expect("variant use through import alias should exist");

    assert_eq!(
        analysis.prepare_rename_at(retry_use),
        Some(ql_analysis::RenameTarget {
            kind: SymbolKind::Variant,
            name: "Retry".to_owned(),
            span: Span::new(retry_use, retry_use + "Retry".len()),
        })
    );
    assert_eq!(
        analysis.rename_at(retry_use, "Repeat"),
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
                ql_analysis::RenameEdit {
                    span: nth_span(source, "Retry", 3),
                    replacement: "Repeat".to_owned(),
                },
            ],
        }))
    );
}

#[test]
fn rename_queries_follow_unique_method_symbols() {
    let source = r#"
struct Counter {
    total: Int,
}

impl Counter {
    fn read(self) -> Int {
        return self.total
    }
}

fn main(counter: Counter) -> Int {
    return counter.read()
}
"#;

    let analysis = analyzed(source);
    let method_use = source
        .rfind(".read")
        .map(|offset| offset + 1)
        .expect("method use should exist");

    assert_eq!(
        analysis.prepare_rename_at(method_use),
        Some(ql_analysis::RenameTarget {
            kind: SymbolKind::Method,
            name: "read".to_owned(),
            span: Span::new(method_use, method_use + "read".len()),
        })
    );
    assert_eq!(
        analysis.rename_at(method_use, "fetch"),
        Ok(Some(ql_analysis::RenameResult {
            kind: SymbolKind::Method,
            old_name: "read".to_owned(),
            new_name: "fetch".to_owned(),
            edits: vec![
                ql_analysis::RenameEdit {
                    span: nth_span(source, "read", 1),
                    replacement: "fetch".to_owned(),
                },
                ql_analysis::RenameEdit {
                    span: Span::new(method_use, method_use + "read".len()),
                    replacement: "fetch".to_owned(),
                },
            ],
        }))
    );
}

#[test]
fn rename_queries_keep_ambiguous_method_surfaces_closed() {
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

    let analysis = analyzed(source);
    let method_use = source
        .rfind(".ping")
        .map(|offset| offset + 1)
        .expect("ambiguous method use should exist");

    assert_eq!(analysis.prepare_rename_at(method_use), None);
    assert_eq!(analysis.rename_at(method_use, "pong"), Ok(None));
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
fn rename_queries_support_fields_and_skip_remaining_unsupported_symbols() {
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

    assert_eq!(
        analysis.prepare_rename_at(import_use),
        Some(ql_analysis::RenameTarget {
            kind: SymbolKind::Import,
            name: "Map".to_owned(),
            span: Span::new(import_use, import_use + "Map".len()),
        })
    );
    assert_eq!(analysis.prepare_rename_at(builtin_use), None);
    assert_eq!(
        analysis.prepare_rename_at(field_use),
        Some(ql_analysis::RenameTarget {
            kind: SymbolKind::Field,
            name: "value".to_owned(),
            span: Span::new(field_use, field_use + "value".len()),
        })
    );
    assert_eq!(
        analysis.prepare_rename_at(method_use),
        Some(ql_analysis::RenameTarget {
            kind: SymbolKind::Method,
            name: "get".to_owned(),
            span: Span::new(method_use, method_use + "get".len()),
        })
    );
    assert_eq!(analysis.prepare_rename_at(self_use), None);

    assert_eq!(
        analysis.rename_at(import_use, "CacheMap"),
        Ok(Some(ql_analysis::RenameResult {
            kind: SymbolKind::Import,
            old_name: "Map".to_owned(),
            new_name: "CacheMap".to_owned(),
            edits: vec![
                ql_analysis::RenameEdit {
                    span: alias_span(source, "Map"),
                    replacement: "CacheMap".to_owned(),
                },
                ql_analysis::RenameEdit {
                    span: Span::new(import_use, import_use + "Map".len()),
                    replacement: "CacheMap".to_owned(),
                },
            ],
        }))
    );
    assert_eq!(
        analysis.rename_at(field_use, "count"),
        Ok(Some(ql_analysis::RenameResult {
            kind: SymbolKind::Field,
            old_name: "value".to_owned(),
            new_name: "count".to_owned(),
            edits: vec![
                ql_analysis::RenameEdit {
                    span: nth_span(source, "value", 1),
                    replacement: "count".to_owned(),
                },
                ql_analysis::RenameEdit {
                    span: Span::new(field_use, field_use + "value".len()),
                    replacement: "count".to_owned(),
                },
            ],
        }))
    );
    assert_eq!(
        analysis.rename_at(method_use, "renamed"),
        Ok(Some(ql_analysis::RenameResult {
            kind: SymbolKind::Method,
            old_name: "get".to_owned(),
            new_name: "renamed".to_owned(),
            edits: vec![
                ql_analysis::RenameEdit {
                    span: nth_span(source, "get", 1),
                    replacement: "renamed".to_owned(),
                },
                ql_analysis::RenameEdit {
                    span: Span::new(method_use, method_use + "get".len()),
                    replacement: "renamed".to_owned(),
                },
            ],
        }))
    );
}

#[test]
fn rename_queries_follow_opaque_type_symbols() {
    let source = r#"
opaque type UserId = Int

struct Account {
    id: UserId,
}

fn build(value: UserId) -> UserId {
    return value
}
"#;

    let analysis = analyzed(source);
    let opaque_use = nth_offset(source, "UserId", 2);

    assert_eq!(
        analysis.prepare_rename_at(opaque_use),
        Some(ql_analysis::RenameTarget {
            kind: SymbolKind::TypeAlias,
            name: "UserId".to_owned(),
            span: nth_span(source, "UserId", 2),
        })
    );
    assert_eq!(
        analysis.rename_at(opaque_use, "AccountId"),
        Ok(Some(ql_analysis::RenameResult {
            kind: SymbolKind::TypeAlias,
            old_name: "UserId".to_owned(),
            new_name: "AccountId".to_owned(),
            edits: vec![
                ql_analysis::RenameEdit {
                    span: nth_span(source, "UserId", 1),
                    replacement: "AccountId".to_owned(),
                },
                ql_analysis::RenameEdit {
                    span: nth_span(source, "UserId", 2),
                    replacement: "AccountId".to_owned(),
                },
                ql_analysis::RenameEdit {
                    span: nth_span(source, "UserId", 3),
                    replacement: "AccountId".to_owned(),
                },
                ql_analysis::RenameEdit {
                    span: nth_span(source, "UserId", 4),
                    replacement: "AccountId".to_owned(),
                },
            ],
        }))
    );
}

#[test]
fn completion_queries_follow_visible_value_bindings_and_shadowing() {
    let source = r#"
use std.collections.HashMap as Map

fn build[T](input: T) -> T {
    let output = input
    if true {
        let input = output
        return input
    }
    return output
}
"#;

    let analysis = analyzed(source);
    let inner_input_use = source
        .rfind("return input")
        .map(|offset| offset + "return ".len())
        .expect("inner return input should exist");
    let items = analysis
        .completions_at(inner_input_use)
        .expect("value completion should exist");

    assert_eq!(
        items
            .iter()
            .map(|item| item.label.as_str())
            .collect::<Vec<_>>(),
        vec!["Map", "build", "input", "output"]
    );
    assert_eq!(items[0].kind, SymbolKind::Import);
    assert_eq!(items[0].detail, "import std.collections.HashMap");
    assert_eq!(items[0].insert_text, "Map");
    assert_eq!(items[1].kind, SymbolKind::Function);
    assert_eq!(items[1].detail, "fn build[T](input: T) -> T");
    assert_eq!(items[1].insert_text, "build");
    assert_eq!(items[2].kind, SymbolKind::Local);
    assert_eq!(items[2].detail, "local input: T");
    assert_eq!(items[2].insert_text, "input");
    assert_eq!(items[3].kind, SymbolKind::Local);
    assert_eq!(items[3].detail, "local output: T");
    assert_eq!(items[3].insert_text, "output");
    assert!(!items.iter().any(|item| item.label == "T"));
}

#[test]
fn completion_queries_follow_value_context_candidate_lists() {
    let source = r#"
use std.collections.HashMap as amap

const bconst: Int = 10
static cstatic: Int = 20

extern "c" {
    fn d_block(left: Int, right: Int) -> Int
}

extern "c" fn e_decl(left: Int, right: Int) -> Int

extern "c" fn f_def(left: Int, right: Int) -> Int {
    return left * right
}

fn g_helper() -> Int {
    return bconst + cstatic
}

fn run(param_value: Int) -> Int {
    let local_value = param_value
    return hole
}
"#;

    let analysis = analyzed(source);
    let value_use = source
        .find("hole\n")
        .expect("return expression should contain the placeholder");
    let items = analysis
        .completions_at(value_use)
        .expect("value completion should exist");

    assert_eq!(
        items
            .iter()
            .map(|item| item.label.as_str())
            .collect::<Vec<_>>(),
        vec![
            "amap",
            "bconst",
            "cstatic",
            "d_block",
            "e_decl",
            "f_def",
            "g_helper",
            "local_value",
            "param_value",
            "run",
        ]
    );
    assert_eq!(items[0].kind, SymbolKind::Import);
    assert_eq!(items[0].detail, "import std.collections.HashMap");
    assert_eq!(items[0].insert_text, "amap");
    assert_eq!(items[1].kind, SymbolKind::Const);
    assert_eq!(items[1].detail, "const bconst: Int");
    assert_eq!(items[1].insert_text, "bconst");
    assert_eq!(items[2].kind, SymbolKind::Static);
    assert_eq!(items[2].detail, "static cstatic: Int");
    assert_eq!(items[2].insert_text, "cstatic");
    assert_eq!(items[3].kind, SymbolKind::Function);
    assert_eq!(
        items[3].detail,
        "extern \"c\" fn d_block(left: Int, right: Int) -> Int"
    );
    assert_eq!(items[3].insert_text, "d_block");
    assert_eq!(items[4].kind, SymbolKind::Function);
    assert_eq!(
        items[4].detail,
        "extern \"c\" fn e_decl(left: Int, right: Int) -> Int"
    );
    assert_eq!(items[4].insert_text, "e_decl");
    assert_eq!(items[5].kind, SymbolKind::Function);
    assert_eq!(
        items[5].detail,
        "extern \"c\" fn f_def(left: Int, right: Int) -> Int"
    );
    assert_eq!(items[5].insert_text, "f_def");
    assert_eq!(items[6].kind, SymbolKind::Function);
    assert_eq!(items[6].detail, "fn g_helper() -> Int");
    assert_eq!(items[6].insert_text, "g_helper");
    assert_eq!(items[7].kind, SymbolKind::Local);
    assert_eq!(items[7].detail, "local local_value: Int");
    assert_eq!(items[7].insert_text, "local_value");
    assert_eq!(items[8].kind, SymbolKind::Parameter);
    assert_eq!(items[8].detail, "param param_value: Int");
    assert_eq!(items[8].insert_text, "param_value");
    assert_eq!(items[9].kind, SymbolKind::Function);
    assert_eq!(items[9].detail, "fn run(param_value: Int) -> Int");
    assert_eq!(items[9].insert_text, "run");
}

#[test]
fn completion_queries_surface_free_function_candidates_in_value_contexts() {
    let source = r#"
fn build(value: Int) -> Int {
    return value
}

fn choose() -> (Int) -> Int {
    return build
}
"#;

    let analysis = analyzed(source);
    let build_use = nth_offset(source, "build", 2);
    let items = analysis
        .completions_at(build_use)
        .expect("value completion should exist");

    assert!(items.iter().any(|item| {
        item.label == "build"
            && item.insert_text == "build"
            && item.kind == SymbolKind::Function
            && item.detail == "fn build(value: Int) -> Int"
    }));
}

#[test]
fn completion_queries_surface_extern_callable_candidates_in_value_contexts() {
    let source = r#"
extern "c" {
    fn q_add(left: Int, right: Int) -> Int
}

extern "c" fn q_sub(left: Int, right: Int) -> Int

extern "c" fn q_mul(left: Int, right: Int) -> Int {
    return left * right
}

fn choose_decl() -> (Int, Int) -> Int {
    return q_ad
}

fn choose_top_level() -> (Int, Int) -> Int {
    return q_su
}

fn choose_definition() -> (Int, Int) -> Int {
    return q_mu
}
"#;

    let analysis = analyzed(source);

    let extern_block_use = source
        .find("q_ad\n")
        .expect("extern block completion site should exist");
    let extern_block_items = analysis
        .completions_at(extern_block_use)
        .expect("extern block completion should exist");
    assert!(extern_block_items.iter().any(|item| {
        item.label == "q_add"
            && item.insert_text == "q_add"
            && item.kind == SymbolKind::Function
            && item.detail == "extern \"c\" fn q_add(left: Int, right: Int) -> Int"
    }));

    let top_level_decl_use = source
        .find("q_su\n")
        .expect("top-level extern declaration completion site should exist");
    let top_level_decl_items = analysis
        .completions_at(top_level_decl_use)
        .expect("top-level extern declaration completion should exist");
    assert!(top_level_decl_items.iter().any(|item| {
        item.label == "q_sub"
            && item.insert_text == "q_sub"
            && item.kind == SymbolKind::Function
            && item.detail == "extern \"c\" fn q_sub(left: Int, right: Int) -> Int"
    }));

    let top_level_def_use = source
        .find("q_mu\n")
        .expect("top-level extern definition completion site should exist");
    let top_level_def_items = analysis
        .completions_at(top_level_def_use)
        .expect("top-level extern definition completion should exist");
    assert!(top_level_def_items.iter().any(|item| {
        item.label == "q_mul"
            && item.insert_text == "q_mul"
            && item.kind == SymbolKind::Function
            && item.detail == "extern \"c\" fn q_mul(left: Int, right: Int) -> Int"
    }));
}

#[test]
fn completion_queries_surface_plain_import_alias_candidates_in_value_contexts() {
    let source = r#"
use std.collections.HashMap as Map

fn build() -> Int {
    let value = Ma
    return 0
}
"#;

    let analysis = analyzed(source);
    let alias_use = source
        .find("Ma\n")
        .expect("local binding should contain the partial import alias");
    let items = analysis
        .completions_at(alias_use)
        .expect("value completion should exist");

    assert!(items.iter().any(|item| {
        item.label == "Map"
            && item.insert_text == "Map"
            && item.kind == SymbolKind::Import
            && item.detail == "import std.collections.HashMap"
    }));
}

#[test]
fn completion_queries_surface_const_and_static_value_candidates_by_prefix() {
    let source = r#"
const LIMIT: Int = 10
static CURRENT: Int = 20

fn build() -> Int {
    return LIM + CURR
}
"#;

    let analysis = analyzed(source);
    let const_use = source
        .find("LIM +")
        .expect("return expression should contain the partial const name");
    let const_items = analysis
        .completions_at(const_use)
        .expect("const completion should exist");
    assert!(const_items.iter().any(|item| {
        item.label == "LIMIT"
            && item.insert_text == "LIMIT"
            && item.kind == SymbolKind::Const
            && item.detail == "const LIMIT: Int"
    }));

    let static_use = source
        .rfind("CURR")
        .expect("return expression should contain the partial static name");
    let static_items = analysis
        .completions_at(static_use)
        .expect("static completion should exist");
    assert!(static_items.iter().any(|item| {
        item.label == "CURRENT"
            && item.insert_text == "CURRENT"
            && item.kind == SymbolKind::Static
            && item.detail == "static CURRENT: Int"
    }));
}

#[test]
fn completion_queries_surface_local_value_candidates_by_prefix() {
    let source = r#"
fn build(seed: Int) -> Int {
    let local_value = seed
    return local_v
}
"#;

    let analysis = analyzed(source);
    let local_use = source
        .find("local_v\n")
        .expect("return expression should contain the partial local name");
    let items = analysis
        .completions_at(local_use)
        .expect("local completion should exist");

    assert!(items.iter().any(|item| {
        item.label == "local_value"
            && item.insert_text == "local_value"
            && item.kind == SymbolKind::Local
            && item.detail == "local local_value: Int"
    }));
}

#[test]
fn completion_queries_surface_parameter_value_candidates_by_prefix() {
    let source = r#"
fn build(value: Int) -> Int {
    return val
}
"#;

    let analysis = analyzed(source);
    let param_use = source
        .find("val\n")
        .expect("return expression should contain the partial parameter name");
    let items = analysis
        .completions_at(param_use)
        .expect("parameter completion should exist");

    assert!(items.iter().any(|item| {
        item.label == "value"
            && item.insert_text == "value"
            && item.kind == SymbolKind::Parameter
            && item.detail == "param value: Int"
    }));
}

#[test]
fn completion_queries_preserve_escaped_insert_text_for_keyword_bindings() {
    let source = r#"
fn keyword_passthrough(`type`: String) -> String {
    return `type`
}
"#;

    let analysis = analyzed(source);
    let use_span = nth_span(source, "`type`", 2);
    let items = analysis
        .completions_at(use_span.start + 3)
        .expect("escaped identifier completion should exist");

    assert!(
        items.iter().any(|item| {
            item.label == "type"
                && item.insert_text == "`type`"
                && item.kind == SymbolKind::Parameter
        }),
        "escaped identifier completion should preserve source-valid insert text"
    );
}

#[test]
fn completion_queries_follow_type_contexts() {
    let source = r#"
use std.collections.HashMap as ZMap

type ZAlias = Int
opaque type ZOpaque = Int

enum ZMode {
    Idle,
}

trait ZReader {}

struct ZUser {}

fn build[ZT](value: ZMap[String, ZT]) -> ZUser {
    return ZUser {}
}
"#;

    let analysis = analyzed(source);
    let type_use = source
        .find("ZMap[String, ZT]")
        .expect("function parameter should contain the import alias");
    let items = analysis
        .completions_at(type_use)
        .expect("type completion should exist");

    assert_eq!(
        items
            .iter()
            .map(|item| item.label.as_str())
            .collect::<Vec<_>>(),
        vec![
            "Bool", "Bytes", "Char", "F32", "F64", "I16", "I32", "I64", "I8", "ISize", "Int",
            "Never", "String", "U16", "U32", "U64", "U8", "UInt", "USize", "Void", "ZAlias",
            "ZMap", "ZMode", "ZOpaque", "ZReader", "ZT", "ZUser"
        ]
    );
    assert_eq!(items[20].kind, SymbolKind::TypeAlias);
    assert_eq!(items[20].detail, "type ZAlias = Int");
    assert_eq!(items[21].kind, SymbolKind::Import);
    assert_eq!(items[21].detail, "import std.collections.HashMap");
    assert_eq!(items[22].kind, SymbolKind::Enum);
    assert_eq!(items[22].detail, "enum ZMode");
    assert_eq!(items[23].kind, SymbolKind::TypeAlias);
    assert_eq!(items[23].detail, "opaque type ZOpaque = Int");
    assert_eq!(items[24].kind, SymbolKind::Trait);
    assert_eq!(items[24].detail, "trait ZReader");
    assert_eq!(items[25].kind, SymbolKind::Generic);
    assert_eq!(items[25].detail, "generic ZT");
    assert_eq!(items[26].kind, SymbolKind::Struct);
    assert_eq!(items[26].detail, "struct ZUser");
    assert!(!items.iter().any(|item| item.label == "value"));
    assert!(!items.iter().any(|item| item.label == "build"));
}

#[test]
fn completion_queries_surface_plain_import_alias_candidates_in_type_contexts() {
    let source = r#"
use std.collections.HashMap as Map

fn build(value: Ma) -> Map[String, Int] {
    return value
}
"#;

    let analysis = analyzed(source);
    let type_use = source
        .find("Ma)")
        .expect("function parameter should contain the partial import alias");
    let items = analysis
        .completions_at(type_use)
        .expect("type completion should exist");

    assert!(items.iter().any(|item| {
        item.label == "Map"
            && item.insert_text == "Map"
            && item.kind == SymbolKind::Import
            && item.detail == "import std.collections.HashMap"
    }));
}

#[test]
fn completion_queries_surface_builtin_and_struct_type_candidates_by_prefix() {
    let source = r#"
struct User {}

fn build(value: Str) -> Us {
    return User {}
}
"#;

    let analysis = analyzed(source);
    let builtin_use = source
        .find("Str)")
        .expect("function parameter should contain the partial builtin type");
    let builtin_items = analysis
        .completions_at(builtin_use)
        .expect("builtin type completion should exist");
    assert!(builtin_items.iter().any(|item| {
        item.label == "String"
            && item.insert_text == "String"
            && item.kind == SymbolKind::BuiltinType
            && item.detail == "builtin type String"
    }));

    let struct_use = source
        .find("-> Us")
        .map(|offset| offset + "-> ".len())
        .expect("function return type should contain the partial struct name");
    let struct_items = analysis
        .completions_at(struct_use)
        .expect("struct type completion should exist");
    assert!(struct_items.iter().any(|item| {
        item.label == "User"
            && item.insert_text == "User"
            && item.kind == SymbolKind::Struct
            && item.detail == "struct User"
    }));
}

#[test]
fn completion_queries_surface_type_alias_candidates_by_prefix() {
    let source = r#"
type IdAlias = Int

fn build(value: IdA) -> IdAlias {
    return value
}
"#;

    let analysis = analyzed(source);
    let alias_use = source
        .find("IdA)")
        .expect("function parameter should contain the partial type alias");
    let items = analysis
        .completions_at(alias_use)
        .expect("type-alias completion should exist");

    assert!(items.iter().any(|item| {
        item.label == "IdAlias"
            && item.insert_text == "IdAlias"
            && item.kind == SymbolKind::TypeAlias
            && item.detail == "type IdAlias = Int"
    }));
}

#[test]
fn completion_queries_surface_opaque_type_candidates_by_prefix() {
    let source = r#"
opaque type UserId = Int

fn build(value: Us) -> UserId {
    return value
}
"#;

    let analysis = analyzed(source);
    let opaque_use = source
        .find("Us)")
        .expect("function parameter should contain the partial opaque type");
    let items = analysis
        .completions_at(opaque_use)
        .expect("opaque-type completion should exist");

    assert!(items.iter().any(|item| {
        item.label == "UserId"
            && item.insert_text == "UserId"
            && item.kind == SymbolKind::TypeAlias
            && item.detail == "opaque type UserId = Int"
    }));
}

#[test]
fn completion_queries_surface_generic_type_candidates_by_prefix() {
    let source = r#"
fn build[ResultType](value: Res) -> ResultType {
    return value
}
"#;

    let analysis = analyzed(source);
    let generic_use = source
        .find("Res)")
        .expect("function parameter should contain the partial generic type");
    let items = analysis
        .completions_at(generic_use)
        .expect("generic type completion should exist");

    assert!(items.iter().any(|item| {
        item.label == "ResultType"
            && item.insert_text == "ResultType"
            && item.kind == SymbolKind::Generic
            && item.detail == "generic ResultType"
    }));
}

#[test]
fn completion_queries_surface_enum_type_candidates_by_prefix() {
    let source = r#"
enum Mode {
    Idle,
}

fn build(value: Mo) -> Mode {
    return value
}
"#;

    let analysis = analyzed(source);

    let enum_use = source
        .find("Mo)")
        .expect("function parameter should contain the partial enum type");
    let enum_items = analysis
        .completions_at(enum_use)
        .expect("enum type completion should exist");
    assert!(enum_items.iter().any(|item| {
        item.label == "Mode"
            && item.insert_text == "Mode"
            && item.kind == SymbolKind::Enum
            && item.detail == "enum Mode"
    }));
}

#[test]
fn completion_queries_surface_trait_type_candidates_by_prefix() {
    let source = r#"
trait Reader {}

fn build(value: Re) -> Reader {
    return value
}
"#;

    let analysis = analyzed(source);

    let trait_use = source
        .find("Re)")
        .expect("function parameter should contain the partial trait type");
    let trait_items = analysis
        .completions_at(trait_use)
        .expect("trait type completion should exist");
    assert!(trait_items.iter().any(|item| {
        item.label == "Reader"
            && item.insert_text == "Reader"
            && item.kind == SymbolKind::Trait
            && item.detail == "trait Reader"
    }));
}

#[test]
fn completion_queries_follow_member_candidates_on_stable_receiver_types() {
    let source = r#"
struct Counter {
    total: Int,
    value: Int,
}

impl Counter {
    fn get(self) -> Int {
        return self.value
    }

    fn read(self) -> Int {
        return self.total
    }
}

extend Counter {
    fn extra(self) -> Int {
        return self.value
    }
}

fn main(counter: Counter) -> Int {
    return counter.read()
}
"#;

    let analysis = analyzed(source);
    let member_use = source
        .rfind(".read")
        .map(|offset| offset + 1)
        .expect("member use should exist");
    let items = analysis
        .completions_at(member_use)
        .expect("member completion should exist");

    assert_eq!(
        items
            .iter()
            .map(|item| item.label.as_str())
            .collect::<Vec<_>>(),
        vec!["extra", "get", "read", "total", "value"]
    );
    assert_eq!(items[0].kind, SymbolKind::Method);
    assert_eq!(items[0].detail, "fn extra(self) -> Int");
    assert_eq!(items[0].insert_text, "extra");
    assert_eq!(items[1].kind, SymbolKind::Method);
    assert_eq!(items[1].detail, "fn get(self) -> Int");
    assert_eq!(items[1].insert_text, "get");
    assert_eq!(items[2].kind, SymbolKind::Method);
    assert_eq!(items[2].detail, "fn read(self) -> Int");
    assert_eq!(items[2].insert_text, "read");
    assert_eq!(items[3].kind, SymbolKind::Field);
    assert_eq!(items[3].detail, "field total: Int");
    assert_eq!(items[3].insert_text, "total");
    assert_eq!(items[4].kind, SymbolKind::Field);
    assert_eq!(items[4].detail, "field value: Int");
    assert_eq!(items[4].insert_text, "value");
}

#[test]
fn completion_queries_surface_field_candidates_on_stable_receiver_types() {
    let source = r#"
struct Counter {
    value: Int,
    total: Int,
}

fn main(counter: Counter) -> Int {
    return counter.va
}
"#;

    let analysis = analyzed(source);
    let member_use = source
        .rfind(".va")
        .map(|offset| offset + 1)
        .expect("member use should exist");
    let items = analysis
        .completions_at(member_use)
        .expect("member completion should exist");

    assert!(items.iter().any(|item| {
        item.label == "value"
            && item.insert_text == "value"
            && item.kind == SymbolKind::Field
            && item.detail == "field value: Int"
    }));
}

#[test]
fn completion_queries_surface_unique_method_candidates_on_stable_receiver_types() {
    let source = r#"
struct Counter {
    value: Int,
}

impl Counter {
    fn read(self, delta: Int) -> Int {
        return self.value + delta
    }
}

fn main(counter: Counter) -> Int {
    return counter.re
}
"#;

    let analysis = analyzed(source);
    let member_use = source
        .rfind(".re")
        .map(|offset| offset + 1)
        .expect("member use should exist");
    let items = analysis
        .completions_at(member_use)
        .expect("member completion should exist");

    assert!(items.iter().any(|item| {
        item.label == "read"
            && item.insert_text == "read"
            && item.kind == SymbolKind::Method
            && item.detail == "fn read(self, delta: Int) -> Int"
    }));
}

#[test]
fn completion_queries_follow_variant_candidates_on_enum_item_roots() {
    let source = r#"
enum Command {
    Retry(Int),
    Stop,
}

fn main() -> Command {
    return Command.Re()
}
"#;

    let analysis = analyzed(source);
    let member_use = source
        .find(".Re")
        .map(|offset| offset + 1)
        .expect("variant path use should exist");
    let items = analysis
        .completions_at(member_use)
        .expect("variant completion should exist");

    assert_eq!(
        items
            .iter()
            .map(|item| item.label.as_str())
            .collect::<Vec<_>>(),
        vec!["Retry", "Stop"]
    );
    assert!(items.iter().all(|item| item.kind == SymbolKind::Variant));
}

#[test]
fn completion_queries_surface_variant_candidates_on_enum_item_roots() {
    let source = r#"
enum Command {
    Retry(Int),
    Stop,
}

fn main() -> Command {
    return Command.Re()
}
"#;

    let analysis = analyzed(source);
    let member_use = source
        .find(".Re")
        .map(|offset| offset + 1)
        .expect("variant path use should exist");
    let items = analysis
        .completions_at(member_use)
        .expect("variant completion should exist");

    assert!(items.iter().any(|item| {
        item.label == "Retry"
            && item.insert_text == "Retry"
            && item.kind == SymbolKind::Variant
            && item.detail == "variant Command.Retry(Int)"
    }));
}

#[test]
fn completion_queries_follow_variant_candidates_in_struct_literal_paths() {
    let source = r#"
enum Command {
    Config { value: Int },
    Stop,
}

fn build() -> Command {
    return Command.Con { value: 1 }
}
"#;

    let analysis = analyzed(source);
    let member_use = source
        .find(".Con")
        .map(|offset| offset + 1)
        .expect("variant struct-literal path should exist");
    let items = analysis
        .completions_at(member_use)
        .expect("variant completion should exist");

    assert_eq!(
        items
            .iter()
            .map(|item| item.label.as_str())
            .collect::<Vec<_>>(),
        vec!["Config", "Stop"]
    );
    assert!(items.iter().all(|item| item.kind == SymbolKind::Variant));
}

#[test]
fn completion_queries_surface_variant_candidates_in_struct_literal_paths() {
    let source = r#"
enum Command {
    Config { value: Int },
    Stop,
}

fn build() -> Command {
    return Command.Con { value: 1 }
}
"#;

    let analysis = analyzed(source);
    let member_use = source
        .find(".Con")
        .map(|offset| offset + 1)
        .expect("variant struct-literal path should exist");
    let items = analysis
        .completions_at(member_use)
        .expect("variant completion should exist");

    assert!(items.iter().any(|item| {
        item.label == "Config"
            && item.insert_text == "Config"
            && item.kind == SymbolKind::Variant
            && item.detail == "variant Command.Config { value: Int }"
    }));
}

#[test]
fn completion_queries_follow_variant_candidates_in_pattern_paths() {
    let source = r#"
enum Command {
    Retry(Int),
    Stop,
}

fn read(command: Command) -> Int {
    return match command {
        Command.Re(value) => value,
        Command.Stop => 0,
    }
}
"#;

    let analysis = analyzed(source);
    let member_use = source
        .find(".Re(")
        .map(|offset| offset + 1)
        .expect("variant pattern path should exist");
    let items = analysis
        .completions_at(member_use)
        .expect("variant completion should exist");

    assert_eq!(
        items
            .iter()
            .map(|item| item.label.as_str())
            .collect::<Vec<_>>(),
        vec!["Retry", "Stop"]
    );
    assert!(items.iter().all(|item| item.kind == SymbolKind::Variant));
}

#[test]
fn completion_queries_surface_variant_candidates_in_pattern_paths() {
    let source = r#"
enum Command {
    Retry(Int),
    Stop,
}

fn read(command: Command) -> Int {
    return match command {
        Command.Re(value) => value,
        Command.Stop => 0,
    }
}
"#;

    let analysis = analyzed(source);
    let member_use = source
        .find(".Re(")
        .map(|offset| offset + 1)
        .expect("variant pattern path should exist");
    let items = analysis
        .completions_at(member_use)
        .expect("variant completion should exist");

    assert!(items.iter().any(|item| {
        item.label == "Retry"
            && item.insert_text == "Retry"
            && item.kind == SymbolKind::Variant
            && item.detail == "variant Command.Retry(Int)"
    }));
}

#[test]
fn completion_queries_follow_variant_candidates_on_same_file_import_alias_roots() {
    let source = r#"
use Command as Cmd

enum Command {
    Retry(Int),
    Stop,
}

fn main() -> Command {
    return Cmd.Re()
}
"#;

    let analysis = analyzed(source);
    let member_use = source
        .find(".Re")
        .map(|offset| offset + 1)
        .expect("variant path through import alias should exist");
    let items = analysis
        .completions_at(member_use)
        .expect("variant completion through import alias should exist");

    assert_eq!(
        items
            .iter()
            .map(|item| item.label.as_str())
            .collect::<Vec<_>>(),
        vec!["Retry", "Stop"]
    );
    assert!(items.iter().all(|item| item.kind == SymbolKind::Variant));
}

#[test]
fn completion_queries_surface_variant_candidates_on_same_file_import_alias_roots() {
    let source = r#"
use Command as Cmd

enum Command {
    Retry(Int),
    Stop,
}

fn main() -> Command {
    return Cmd.Re()
}
"#;

    let analysis = analyzed(source);
    let member_use = source
        .find(".Re")
        .map(|offset| offset + 1)
        .expect("variant path through import alias should exist");
    let items = analysis
        .completions_at(member_use)
        .expect("variant completion through import alias should exist");

    assert!(items.iter().any(|item| {
        item.label == "Retry"
            && item.insert_text == "Retry"
            && item.kind == SymbolKind::Variant
            && item.detail == "variant Command.Retry(Int)"
    }));
}

#[test]
fn completion_queries_follow_variant_candidates_in_import_alias_struct_literal_paths() {
    let source = r#"
use Command as Cmd

enum Command {
    Config { value: Int },
    Stop,
}

fn build() -> Command {
    return Cmd.Con { value: 1 }
}
"#;

    let analysis = analyzed(source);
    let member_use = source
        .find(".Con")
        .map(|offset| offset + 1)
        .expect("struct-literal variant path through import alias should exist");
    let items = analysis
        .completions_at(member_use)
        .expect("variant completion through import alias should exist");

    assert_eq!(
        items
            .iter()
            .map(|item| item.label.as_str())
            .collect::<Vec<_>>(),
        vec!["Config", "Stop"]
    );
    assert!(items.iter().all(|item| item.kind == SymbolKind::Variant));
}

#[test]
fn completion_queries_surface_variant_candidates_in_import_alias_struct_literal_paths() {
    let source = r#"
use Command as Cmd

enum Command {
    Config { value: Int },
    Stop,
}

fn build() -> Command {
    return Cmd.Con { value: 1 }
}
"#;

    let analysis = analyzed(source);
    let member_use = source
        .find(".Con")
        .map(|offset| offset + 1)
        .expect("struct-literal variant path through import alias should exist");
    let items = analysis
        .completions_at(member_use)
        .expect("variant completion through import alias should exist");

    assert!(items.iter().any(|item| {
        item.label == "Config"
            && item.insert_text == "Config"
            && item.kind == SymbolKind::Variant
            && item.detail == "variant Command.Config { value: Int }"
    }));
}

#[test]
fn completion_queries_follow_variant_candidates_in_import_alias_pattern_paths() {
    let source = r#"
use Command as Cmd

enum Command {
    Retry(Int),
    Stop,
}

fn read(command: Command) -> Int {
    return match command {
        Cmd.Re(value) => value,
        Cmd.Stop => 0,
    }
}
"#;

    let analysis = analyzed(source);
    let member_use = source
        .find(".Re(")
        .map(|offset| offset + 1)
        .expect("variant pattern path through import alias should exist");
    let items = analysis
        .completions_at(member_use)
        .expect("variant completion through import alias should exist");

    assert_eq!(
        items
            .iter()
            .map(|item| item.label.as_str())
            .collect::<Vec<_>>(),
        vec!["Retry", "Stop"]
    );
    assert!(items.iter().all(|item| item.kind == SymbolKind::Variant));
}

#[test]
fn completion_queries_surface_variant_candidates_in_import_alias_pattern_paths() {
    let source = r#"
use Command as Cmd

enum Command {
    Retry(Int),
    Stop,
}

fn read(command: Command) -> Int {
    return match command {
        Cmd.Re(value) => value,
        Cmd.Stop => 0,
    }
}
"#;

    let analysis = analyzed(source);
    let member_use = source
        .find(".Re(")
        .map(|offset| offset + 1)
        .expect("variant pattern path through import alias should exist");
    let items = analysis
        .completions_at(member_use)
        .expect("variant completion through import alias should exist");

    assert!(items.iter().any(|item| {
        item.label == "Retry"
            && item.insert_text == "Retry"
            && item.kind == SymbolKind::Variant
            && item.detail == "variant Command.Retry(Int)"
    }));
}

#[test]
fn completion_queries_surface_variant_candidate_lists_across_supported_paths() {
    #[derive(Clone, Copy)]
    struct ExpectedCompletion<'a> {
        label: &'a str,
        insert_text: &'a str,
        kind: SymbolKind,
        detail: &'a str,
    }

    struct VariantCompletionCase<'a> {
        name: &'a str,
        source: &'a str,
        marker: &'a str,
        expected: &'a [ExpectedCompletion<'a>],
    }

    let cases = [
        VariantCompletionCase {
            name: "enum item root",
            source: r#"
enum Command {
    Retry(Int),
    Stop,
}

fn main() -> Command {
    return Command.Re()
}
"#,
            marker: ".Re",
            expected: &[
                ExpectedCompletion {
                    label: "Retry",
                    insert_text: "Retry",
                    kind: SymbolKind::Variant,
                    detail: "variant Command.Retry(Int)",
                },
                ExpectedCompletion {
                    label: "Stop",
                    insert_text: "Stop",
                    kind: SymbolKind::Variant,
                    detail: "variant Command.Stop",
                },
            ],
        },
        VariantCompletionCase {
            name: "struct literal path",
            source: r#"
enum Command {
    Config { value: Int },
    Stop,
}

fn build() -> Command {
    return Command.Con { value: 1 }
}
"#,
            marker: ".Con",
            expected: &[
                ExpectedCompletion {
                    label: "Config",
                    insert_text: "Config",
                    kind: SymbolKind::Variant,
                    detail: "variant Command.Config { value: Int }",
                },
                ExpectedCompletion {
                    label: "Stop",
                    insert_text: "Stop",
                    kind: SymbolKind::Variant,
                    detail: "variant Command.Stop",
                },
            ],
        },
        VariantCompletionCase {
            name: "pattern path",
            source: r#"
enum Command {
    Retry(Int),
    Stop,
}

fn read(command: Command) -> Int {
    return match command {
        Command.Re(value) => value,
        Command.Stop => 0,
    }
}
"#,
            marker: ".Re(",
            expected: &[
                ExpectedCompletion {
                    label: "Retry",
                    insert_text: "Retry",
                    kind: SymbolKind::Variant,
                    detail: "variant Command.Retry(Int)",
                },
                ExpectedCompletion {
                    label: "Stop",
                    insert_text: "Stop",
                    kind: SymbolKind::Variant,
                    detail: "variant Command.Stop",
                },
            ],
        },
        VariantCompletionCase {
            name: "import alias root",
            source: r#"
use Command as Cmd

enum Command {
    Retry(Int),
    Stop,
}

fn main() -> Command {
    return Cmd.Re()
}
"#,
            marker: ".Re",
            expected: &[
                ExpectedCompletion {
                    label: "Retry",
                    insert_text: "Retry",
                    kind: SymbolKind::Variant,
                    detail: "variant Command.Retry(Int)",
                },
                ExpectedCompletion {
                    label: "Stop",
                    insert_text: "Stop",
                    kind: SymbolKind::Variant,
                    detail: "variant Command.Stop",
                },
            ],
        },
        VariantCompletionCase {
            name: "import alias struct literal path",
            source: r#"
use Command as Cmd

enum Command {
    Config { value: Int },
    Stop,
}

fn build() -> Command {
    return Cmd.Con { value: 1 }
}
"#,
            marker: ".Con",
            expected: &[
                ExpectedCompletion {
                    label: "Config",
                    insert_text: "Config",
                    kind: SymbolKind::Variant,
                    detail: "variant Command.Config { value: Int }",
                },
                ExpectedCompletion {
                    label: "Stop",
                    insert_text: "Stop",
                    kind: SymbolKind::Variant,
                    detail: "variant Command.Stop",
                },
            ],
        },
        VariantCompletionCase {
            name: "import alias pattern path",
            source: r#"
use Command as Cmd

enum Command {
    Retry(Int),
    Stop,
}

fn read(command: Command) -> Int {
    return match command {
        Cmd.Re(value) => value,
        Cmd.Stop => 0,
    }
}
"#,
            marker: ".Re(",
            expected: &[
                ExpectedCompletion {
                    label: "Retry",
                    insert_text: "Retry",
                    kind: SymbolKind::Variant,
                    detail: "variant Command.Retry(Int)",
                },
                ExpectedCompletion {
                    label: "Stop",
                    insert_text: "Stop",
                    kind: SymbolKind::Variant,
                    detail: "variant Command.Stop",
                },
            ],
        },
    ];

    for case in cases {
        let analysis = analyzed(case.source);
        let member_use = case
            .source
            .find(case.marker)
            .map(|offset| offset + 1)
            .expect("variant path use should exist");
        let items = analysis
            .completions_at(member_use)
            .expect("variant completion should exist");
        let projected = items
            .iter()
            .map(|item| {
                (
                    item.label.clone(),
                    item.insert_text.clone(),
                    item.kind,
                    item.detail.clone(),
                )
            })
            .collect::<Vec<_>>();
        let expected = case
            .expected
            .iter()
            .map(|item| {
                (
                    item.label.to_owned(),
                    item.insert_text.to_owned(),
                    item.kind,
                    item.detail.to_owned(),
                )
            })
            .collect::<Vec<_>>();

        assert_eq!(projected, expected, "{}", case.name);
    }
}

#[test]
fn member_completion_prefers_impl_methods_and_skips_ambiguous_extend_candidates() {
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
    return counter.read(1)
}
"#;

    let analysis = analyzed(source);
    let member_use = source
        .rfind(".read")
        .map(|offset| offset + 1)
        .expect("member use should exist");
    let items = analysis
        .completions_at(member_use)
        .expect("member completion should exist");

    assert_eq!(items.len(), 2);
    assert_eq!(items[0].label, "read");
    assert_eq!(items[0].insert_text, "read");
    assert_eq!(items[0].kind, SymbolKind::Method);
    assert_eq!(items[0].detail, "fn read(self, delta: Int) -> Int");
    assert_eq!(items[1].label, "value");
    assert_eq!(items[1].insert_text, "value");
    assert_eq!(items[1].kind, SymbolKind::Field);
    assert_eq!(items[1].detail, "field value: Int");
    assert!(!items.iter().any(|item| item.label == "ping"));
}

#[test]
fn semantic_tokens_follow_current_query_surface() {
    let source = r#"
use std.collections.HashMap as Map

struct Counter {
    value: Int,
}

impl Counter {
    fn get(self, cache: Map[String, Int]) -> Int {
        return self.value + self.get() + cache.len()
    }
}
"#;

    let analysis = analyzed(source);
    let field_use = source
        .find(".value")
        .map(|offset| offset + 1)
        .expect("field use should exist");
    let method_use = source
        .find(".get")
        .map(|offset| offset + 1)
        .expect("method use should exist");
    let builtin_use = nth_span(source, "String", 1);
    let import_use = source
        .find("Map[String, Int]")
        .expect("import alias use should exist");
    let tokens = analysis.semantic_tokens();

    assert!(tokens.contains(&ql_analysis::SemanticTokenOccurrence {
        span: alias_span(source, "Map"),
        kind: SymbolKind::Import,
    }));
    assert!(tokens.contains(&ql_analysis::SemanticTokenOccurrence {
        span: Span::new(import_use, import_use + "Map".len()),
        kind: SymbolKind::Import,
    }));
    assert!(tokens.contains(&ql_analysis::SemanticTokenOccurrence {
        span: builtin_use,
        kind: SymbolKind::BuiltinType,
    }));
    assert!(tokens.contains(&ql_analysis::SemanticTokenOccurrence {
        span: Span::new(field_use, field_use + "value".len()),
        kind: SymbolKind::Field,
    }));
    assert!(tokens.contains(&ql_analysis::SemanticTokenOccurrence {
        span: Span::new(method_use, method_use + "get".len()),
        kind: SymbolKind::Method,
    }));
}

#[test]
fn semantic_tokens_follow_import_alias_surface() {
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
    let tokens = analysis.semantic_tokens();

    assert!(tokens.contains(&ql_analysis::SemanticTokenOccurrence {
        span: alias_span(source, "Map"),
        kind: SymbolKind::Import,
    }));
    assert!(tokens.contains(&ql_analysis::SemanticTokenOccurrence {
        span: Span::new(first_use, first_use + "Map".len()),
        kind: SymbolKind::Import,
    }));
    assert!(tokens.contains(&ql_analysis::SemanticTokenOccurrence {
        span: Span::new(second_use, second_use + "Map".len()),
        kind: SymbolKind::Import,
    }));
}

#[test]
fn semantic_tokens_follow_lexical_semantic_symbol_surface() {
    let source = r#"
fn id[T](param: T) -> T {
    let local_value = param
    return local_value
}

struct Counter {
    value: String,
}

impl Counter {
    fn read(self, input: String) -> String {
        let alias = input
        return self.value
    }
}
"#;

    let analysis = analyzed(source);
    let self_use = source.find("self.value").expect("self use should exist");
    let builtin_use = nth_span(source, "String", 2);
    let tokens = analysis.semantic_tokens();

    assert!(tokens.contains(&ql_analysis::SemanticTokenOccurrence {
        span: nth_span(source, "T", 1),
        kind: SymbolKind::Generic,
    }));
    assert!(tokens.contains(&ql_analysis::SemanticTokenOccurrence {
        span: nth_span(source, "T", 2),
        kind: SymbolKind::Generic,
    }));
    assert!(tokens.contains(&ql_analysis::SemanticTokenOccurrence {
        span: nth_span(source, "T", 3),
        kind: SymbolKind::Generic,
    }));
    assert!(tokens.contains(&ql_analysis::SemanticTokenOccurrence {
        span: nth_span(source, "param", 1),
        kind: SymbolKind::Parameter,
    }));
    assert!(tokens.contains(&ql_analysis::SemanticTokenOccurrence {
        span: nth_span(source, "param", 2),
        kind: SymbolKind::Parameter,
    }));
    assert!(tokens.contains(&ql_analysis::SemanticTokenOccurrence {
        span: nth_span(source, "local_value", 1),
        kind: SymbolKind::Local,
    }));
    assert!(tokens.contains(&ql_analysis::SemanticTokenOccurrence {
        span: nth_span(source, "local_value", 2),
        kind: SymbolKind::Local,
    }));
    assert!(tokens.contains(&ql_analysis::SemanticTokenOccurrence {
        span: nth_span(source, "self", 1),
        kind: SymbolKind::SelfParameter,
    }));
    assert!(tokens.contains(&ql_analysis::SemanticTokenOccurrence {
        span: ql_span::Span::new(self_use, self_use + "self".len()),
        kind: SymbolKind::SelfParameter,
    }));
    assert!(tokens.contains(&ql_analysis::SemanticTokenOccurrence {
        span: builtin_use,
        kind: SymbolKind::BuiltinType,
    }));
}

#[test]
fn semantic_tokens_follow_import_alias_variant_surface() {
    let source = r#"
use Command as Cmd

enum Command {
    Retry(Int),
    Config { retries: Int },
}

fn build(flag: Bool) -> Command {
    if flag {
        return Cmd.Retry(1)
    }
    return Cmd.Config { retries: 2 }
}
"#;

    let analysis = analyzed(source);
    let retry_use = source
        .find("Cmd.Retry(")
        .map(|offset| offset + "Cmd.".len())
        .expect("retry variant use through import alias should exist");
    let config_use = source
        .find("Cmd.Config {")
        .map(|offset| offset + "Cmd.".len())
        .expect("config variant use through import alias should exist");
    let tokens = analysis.semantic_tokens();

    assert!(tokens.contains(&ql_analysis::SemanticTokenOccurrence {
        span: alias_span(source, "Cmd"),
        kind: SymbolKind::Import,
    }));
    assert!(tokens.contains(&ql_analysis::SemanticTokenOccurrence {
        span: Span::new(retry_use, retry_use + "Retry".len()),
        kind: SymbolKind::Variant,
    }));
    assert!(tokens.contains(&ql_analysis::SemanticTokenOccurrence {
        span: Span::new(config_use, config_use + "Config".len()),
        kind: SymbolKind::Variant,
    }));
}

#[test]
fn semantic_tokens_follow_direct_variant_surface() {
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
"#;

    let analysis = analyzed(source);
    let retry_use = source
        .find("Command.Retry(")
        .map(|offset| offset + "Command.".len())
        .expect("retry variant use should exist");
    let config_use = source
        .find("Command.Config {")
        .map(|offset| offset + "Command.".len())
        .expect("config variant use should exist");
    let tokens = analysis.semantic_tokens();

    assert!(tokens.contains(&ql_analysis::SemanticTokenOccurrence {
        span: nth_span(source, "Retry", 1),
        kind: SymbolKind::Variant,
    }));
    assert!(tokens.contains(&ql_analysis::SemanticTokenOccurrence {
        span: Span::new(retry_use, retry_use + "Retry".len()),
        kind: SymbolKind::Variant,
    }));
    assert!(tokens.contains(&ql_analysis::SemanticTokenOccurrence {
        span: nth_span(source, "Config", 1),
        kind: SymbolKind::Variant,
    }));
    assert!(tokens.contains(&ql_analysis::SemanticTokenOccurrence {
        span: Span::new(config_use, config_use + "Config".len()),
        kind: SymbolKind::Variant,
    }));
}

#[test]
fn semantic_tokens_follow_import_alias_struct_field_surface() {
    let source = r#"
use Point as P

struct Point {
    x: Int,
    y: Int,
}

fn read(point: Point, value: Int) -> Int {
    let built = P { x: value, y: 1 }
    match point {
        P { x: alias, y: 2 } => alias,
    }
    return point.x
}
"#;

    let analysis = analyzed(source);
    let tokens = analysis.semantic_tokens();

    assert!(tokens.contains(&ql_analysis::SemanticTokenOccurrence {
        span: alias_span(source, "P"),
        kind: SymbolKind::Import,
    }));
    assert!(tokens.contains(&ql_analysis::SemanticTokenOccurrence {
        span: nth_span(source, "x", 1),
        kind: SymbolKind::Field,
    }));
    assert!(tokens.contains(&ql_analysis::SemanticTokenOccurrence {
        span: nth_span(source, "x", 2),
        kind: SymbolKind::Field,
    }));
    assert!(tokens.contains(&ql_analysis::SemanticTokenOccurrence {
        span: nth_span(source, "x", 3),
        kind: SymbolKind::Field,
    }));
    assert!(tokens.contains(&ql_analysis::SemanticTokenOccurrence {
        span: nth_span(source, "x", 4),
        kind: SymbolKind::Field,
    }));
}

#[test]
fn semantic_tokens_follow_direct_struct_field_surface() {
    let source = r#"
struct Point {
    x: Int,
    y: Int,
}

fn read(point: Point, value: Int) -> Int {
    let built = Point { x: value, y: 1 }
    match point {
        Point { x: alias, y: 2 } => alias,
    }
    return point.x
}
"#;

    let analysis = analyzed(source);
    let tokens = analysis.semantic_tokens();

    assert!(tokens.contains(&ql_analysis::SemanticTokenOccurrence {
        span: nth_span(source, "x", 1),
        kind: SymbolKind::Field,
    }));
    assert!(tokens.contains(&ql_analysis::SemanticTokenOccurrence {
        span: nth_span(source, "x", 2),
        kind: SymbolKind::Field,
    }));
    assert!(tokens.contains(&ql_analysis::SemanticTokenOccurrence {
        span: nth_span(source, "x", 3),
        kind: SymbolKind::Field,
    }));
    assert!(tokens.contains(&ql_analysis::SemanticTokenOccurrence {
        span: nth_span(source, "x", 4),
        kind: SymbolKind::Field,
    }));
}

#[test]
fn semantic_tokens_follow_same_file_direct_symbol_surface() {
    let source = r#"
enum Command {
    Retry(Int),
    Config { retries: Int },
}

struct Point {
    x: Int,
    y: Int,
}

fn build(flag: Bool) -> Command {
    if flag {
        return Command.Retry(1)
    }
    return Command.Config { retries: 2 }
}

fn project(point: Point, value: Int) -> Int {
    let built = Point { x: value, y: 1 }
    match point {
        Point { x: alias, y: 2 } => alias,
    }
    return 0
}
"#;

    let analysis = analyzed(source);
    let tokens = analysis.semantic_tokens();
    let cases = [
        ("Retry", SymbolKind::Variant, vec![1, 2]),
        ("Config", SymbolKind::Variant, vec![1, 2]),
        ("x", SymbolKind::Field, vec![1, 2, 3]),
    ];

    for (name, kind, occurrences) in cases {
        for occurrence in occurrences {
            assert!(
                tokens.contains(&ql_analysis::SemanticTokenOccurrence {
                    span: nth_span(source, name, occurrence),
                    kind,
                }),
                "{} occurrence {}",
                name,
                occurrence
            );
        }
    }
}

#[test]
fn semantic_tokens_follow_direct_member_surface() {
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
    let field_use = source
        .find(".value")
        .map(|offset| offset + 1)
        .expect("field use should exist");
    let method_use = source
        .find(".get")
        .map(|offset| offset + 1)
        .expect("method use should exist");
    let tokens = analysis.semantic_tokens();

    assert!(tokens.contains(&ql_analysis::SemanticTokenOccurrence {
        span: nth_span(source, "value", 1),
        kind: SymbolKind::Field,
    }));
    assert!(tokens.contains(&ql_analysis::SemanticTokenOccurrence {
        span: Span::new(field_use, field_use + "value".len()),
        kind: SymbolKind::Field,
    }));
    assert!(tokens.contains(&ql_analysis::SemanticTokenOccurrence {
        span: nth_span(source, "get", 1),
        kind: SymbolKind::Method,
    }));
    assert!(tokens.contains(&ql_analysis::SemanticTokenOccurrence {
        span: Span::new(method_use, method_use + "get".len()),
        kind: SymbolKind::Method,
    }));
}

#[test]
fn semantic_tokens_follow_same_file_direct_member_surface() {
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
    let tokens = analysis.semantic_tokens();
    let cases = [
        ("value", SymbolKind::Field, vec![1, 2]),
        ("get", SymbolKind::Method, vec![1, 2]),
    ];

    for (name, kind, occurrences) in cases {
        for occurrence in occurrences {
            assert!(
                tokens.contains(&ql_analysis::SemanticTokenOccurrence {
                    span: nth_span(source, name, occurrence),
                    kind,
                }),
                "{} occurrence {}",
                name,
                occurrence
            );
        }
    }
}

#[test]
fn semantic_tokens_follow_type_namespace_item_surface() {
    let source = r#"
type IdAlias = Int

struct Account {
    id: IdAlias,
}

enum Mode {
    Ready,
}

trait Taggable {
    fn mode(self) -> Mode
}

impl Taggable for Account {
    fn mode(self) -> Mode {
        return Mode.Ready
    }
}

fn build(account: Account, value: IdAlias) -> Mode {
    let copy = value
    return account.mode()
}
"#;

    let analysis = analyzed(source);
    let tokens = analysis.semantic_tokens();

    assert!(tokens.contains(&ql_analysis::SemanticTokenOccurrence {
        span: nth_span(source, "IdAlias", 1),
        kind: SymbolKind::TypeAlias,
    }));
    assert!(tokens.contains(&ql_analysis::SemanticTokenOccurrence {
        span: nth_span(source, "IdAlias", 2),
        kind: SymbolKind::TypeAlias,
    }));
    assert!(tokens.contains(&ql_analysis::SemanticTokenOccurrence {
        span: nth_span(source, "Account", 1),
        kind: SymbolKind::Struct,
    }));
    assert!(tokens.contains(&ql_analysis::SemanticTokenOccurrence {
        span: nth_span(source, "Account", 2),
        kind: SymbolKind::Struct,
    }));
    assert!(tokens.contains(&ql_analysis::SemanticTokenOccurrence {
        span: nth_span(source, "Mode", 1),
        kind: SymbolKind::Enum,
    }));
    assert!(tokens.contains(&ql_analysis::SemanticTokenOccurrence {
        span: nth_span(source, "Mode", 2),
        kind: SymbolKind::Enum,
    }));
    assert!(tokens.contains(&ql_analysis::SemanticTokenOccurrence {
        span: nth_span(source, "Taggable", 1),
        kind: SymbolKind::Trait,
    }));
    assert!(tokens.contains(&ql_analysis::SemanticTokenOccurrence {
        span: nth_span(source, "Taggable", 2),
        kind: SymbolKind::Trait,
    }));
}

#[test]
fn semantic_tokens_follow_opaque_type_surface() {
    let source = r#"
opaque type UserId = Int

struct Account {
    id: UserId,
}

fn build(value: UserId) -> UserId {
    return value
}
"#;

    let analysis = analyzed(source);
    let tokens = analysis.semantic_tokens();

    for occurrence in 1..=4 {
        assert!(tokens.contains(&ql_analysis::SemanticTokenOccurrence {
            span: nth_span(source, "UserId", occurrence),
            kind: SymbolKind::TypeAlias,
        }));
    }
}

#[test]
fn semantic_tokens_follow_same_file_type_namespace_item_surface() {
    let source = r#"
type IdAlias = Int

opaque type UserId = Int

struct Account {
    id: UserId,
    alias: IdAlias,
}

enum Mode {
    Ready,
}

trait Taggable {
    fn mode(self) -> Mode
}

impl Taggable for Account {
    fn mode(self) -> Mode {
        return Mode.Ready
    }
}

fn build(account: Account, user_id: UserId, alias: IdAlias) -> Mode {
    let copied_id = user_id
    let copied_alias = alias
    return account.mode()
}
"#;

    let analysis = analyzed(source);
    let tokens = analysis.semantic_tokens();
    let cases = [
        ("IdAlias", SymbolKind::TypeAlias, vec![1, 2, 3]),
        ("UserId", SymbolKind::TypeAlias, vec![1, 2, 3]),
        ("Account", SymbolKind::Struct, vec![1, 2, 3]),
        ("Mode", SymbolKind::Enum, vec![1, 2, 3, 4, 5]),
        ("Taggable", SymbolKind::Trait, vec![1, 2]),
    ];

    for (name, kind, occurrences) in cases {
        for occurrence in occurrences {
            assert!(
                tokens.contains(&ql_analysis::SemanticTokenOccurrence {
                    span: nth_span(source, name, occurrence),
                    kind,
                }),
                "{} occurrence {}",
                name,
                occurrence
            );
        }
    }
}

#[test]
fn semantic_tokens_follow_global_value_item_surface() {
    let source = r#"
const LIMIT: Int = 10

static CURRENT: Int = LIMIT

fn read() -> Int {
    let snapshot = CURRENT
    return LIMIT
}
"#;

    let analysis = analyzed(source);
    let tokens = analysis.semantic_tokens();

    assert!(tokens.contains(&ql_analysis::SemanticTokenOccurrence {
        span: nth_span(source, "LIMIT", 1),
        kind: SymbolKind::Const,
    }));
    assert!(tokens.contains(&ql_analysis::SemanticTokenOccurrence {
        span: nth_span(source, "LIMIT", 2),
        kind: SymbolKind::Const,
    }));
    assert!(tokens.contains(&ql_analysis::SemanticTokenOccurrence {
        span: nth_span(source, "LIMIT", 3),
        kind: SymbolKind::Const,
    }));
    assert!(tokens.contains(&ql_analysis::SemanticTokenOccurrence {
        span: nth_span(source, "CURRENT", 1),
        kind: SymbolKind::Static,
    }));
    assert!(tokens.contains(&ql_analysis::SemanticTokenOccurrence {
        span: nth_span(source, "CURRENT", 2),
        kind: SymbolKind::Static,
    }));
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

#[test]
fn top_level_extern_function_queries_follow_callable_declarations() {
    let source = r#"
extern "c" fn q_add(left: Int, right: Int) -> Int

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

#[test]
fn top_level_extern_function_definition_queries_follow_callable_symbols() {
    let source = r#"
extern "c" fn q_add(left: Int, right: Int) -> Int {
    return left + right
}

fn main() -> Int {
    return q_add(1, 2)
}
"#;

    let analysis = analyzed(source);
    let hover = analysis
        .hover_at(nth_offset(source, "q_add", 2))
        .expect("extern definition call should hover");

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

#[test]
fn callable_queries_follow_same_file_callable_surface() {
    struct CallableCase<'a> {
        name: &'a str,
        use_occurrence: usize,
        detail: &'a str,
        reference_occurrences: &'a [usize],
    }

    let source = r#"
extern "c" {
    fn q_add(left: Int, right: Int) -> Int
}

extern "c" fn q_sub(left: Int, right: Int) -> Int

extern "c" fn q_mul(left: Int, right: Int) -> Int {
    return left * right
}

fn helper(value: Int) -> Int {
    return value
}

fn compute() -> Int {
    return q_add(1, 2) + q_sub(1, 2) + q_mul(1, 2) + helper(1) + helper(2)
}
"#;

    let analysis = analyzed(source);
    let cases = [
        CallableCase {
            name: "q_add",
            use_occurrence: 2,
            detail: "extern \"c\" fn q_add(left: Int, right: Int) -> Int",
            reference_occurrences: &[1, 2],
        },
        CallableCase {
            name: "q_sub",
            use_occurrence: 2,
            detail: "extern \"c\" fn q_sub(left: Int, right: Int) -> Int",
            reference_occurrences: &[1, 2],
        },
        CallableCase {
            name: "q_mul",
            use_occurrence: 2,
            detail: "extern \"c\" fn q_mul(left: Int, right: Int) -> Int",
            reference_occurrences: &[1, 2],
        },
        CallableCase {
            name: "helper",
            use_occurrence: 2,
            detail: "fn helper(value: Int) -> Int",
            reference_occurrences: &[1, 2, 3],
        },
    ];

    for case in cases {
        let use_offset = nth_offset(source, case.name, case.use_occurrence);
        let hover = analysis
            .hover_at(use_offset)
            .expect("callable use should hover");

        assert_eq!(hover.kind, SymbolKind::Function, "{}", case.name);
        assert_eq!(hover.detail, case.detail, "{}", case.name);
        assert_eq!(
            hover.definition_span,
            Some(nth_span(source, case.name, 1)),
            "{}",
            case.name
        );
        assert_eq!(
            analysis.definition_at(use_offset),
            Some(ql_analysis::DefinitionTarget {
                kind: SymbolKind::Function,
                name: case.name.to_owned(),
                span: nth_span(source, case.name, 1),
            }),
            "{}",
            case.name
        );
        assert_eq!(
            analysis.references_at(use_offset),
            Some(
                case.reference_occurrences
                    .iter()
                    .map(|occurrence| ql_analysis::ReferenceTarget {
                        kind: SymbolKind::Function,
                        name: case.name.to_owned(),
                        span: nth_span(source, case.name, *occurrence),
                        is_definition: *occurrence == 1,
                    })
                    .collect::<Vec<_>>()
            ),
            "{}",
            case.name
        );
    }
}

#[test]
fn rename_queries_follow_extern_block_function_symbols() {
    let source = r#"
extern "c" {
    fn q_add(left: Int, right: Int) -> Int
}

fn main() -> Int {
    return q_add(1, 2)
}
"#;

    let analysis = analyzed(source);
    let extern_use = nth_offset(source, "q_add", 2);

    assert_eq!(
        analysis.prepare_rename_at(extern_use),
        Some(ql_analysis::RenameTarget {
            kind: SymbolKind::Function,
            name: "q_add".to_owned(),
            span: nth_span(source, "q_add", 2),
        })
    );
    assert_eq!(
        analysis.rename_at(extern_use, "q_sum"),
        Ok(Some(ql_analysis::RenameResult {
            kind: SymbolKind::Function,
            old_name: "q_add".to_owned(),
            new_name: "q_sum".to_owned(),
            edits: vec![
                ql_analysis::RenameEdit {
                    span: nth_span(source, "q_add", 1),
                    replacement: "q_sum".to_owned(),
                },
                ql_analysis::RenameEdit {
                    span: nth_span(source, "q_add", 2),
                    replacement: "q_sum".to_owned(),
                },
            ],
        }))
    );
}

#[test]
fn rename_queries_follow_top_level_extern_function_symbols() {
    let source = r#"
extern "c" fn q_add(left: Int, right: Int) -> Int

fn main() -> Int {
    return q_add(1, 2)
}
"#;

    let analysis = analyzed(source);
    let extern_use = nth_offset(source, "q_add", 2);

    assert_eq!(
        analysis.prepare_rename_at(extern_use),
        Some(ql_analysis::RenameTarget {
            kind: SymbolKind::Function,
            name: "q_add".to_owned(),
            span: nth_span(source, "q_add", 2),
        })
    );
    assert_eq!(
        analysis.rename_at(extern_use, "q_sum"),
        Ok(Some(ql_analysis::RenameResult {
            kind: SymbolKind::Function,
            old_name: "q_add".to_owned(),
            new_name: "q_sum".to_owned(),
            edits: vec![
                ql_analysis::RenameEdit {
                    span: nth_span(source, "q_add", 1),
                    replacement: "q_sum".to_owned(),
                },
                ql_analysis::RenameEdit {
                    span: nth_span(source, "q_add", 2),
                    replacement: "q_sum".to_owned(),
                },
            ],
        }))
    );
}

#[test]
fn rename_queries_follow_top_level_extern_function_definition_symbols() {
    let source = r#"
extern "c" fn q_add(left: Int, right: Int) -> Int {
    return left + right
}

fn main() -> Int {
    return q_add(1, 2)
}
"#;

    let analysis = analyzed(source);
    let extern_use = nth_offset(source, "q_add", 2);

    assert_eq!(
        analysis.prepare_rename_at(extern_use),
        Some(ql_analysis::RenameTarget {
            kind: SymbolKind::Function,
            name: "q_add".to_owned(),
            span: nth_span(source, "q_add", 2),
        })
    );
    assert_eq!(
        analysis.rename_at(extern_use, "q_sum"),
        Ok(Some(ql_analysis::RenameResult {
            kind: SymbolKind::Function,
            old_name: "q_add".to_owned(),
            new_name: "q_sum".to_owned(),
            edits: vec![
                ql_analysis::RenameEdit {
                    span: nth_span(source, "q_add", 1),
                    replacement: "q_sum".to_owned(),
                },
                ql_analysis::RenameEdit {
                    span: nth_span(source, "q_add", 2),
                    replacement: "q_sum".to_owned(),
                },
            ],
        }))
    );
}

#[test]
fn semantic_tokens_follow_extern_block_function_surface() {
    let source = r#"
extern "c" {
    fn q_add(left: Int, right: Int) -> Int
}

fn main() -> Int {
    return q_add(1, 2)
}
"#;

    let analysis = analyzed(source);
    let tokens = analysis.semantic_tokens();

    assert!(tokens.contains(&ql_analysis::SemanticTokenOccurrence {
        span: nth_span(source, "q_add", 1),
        kind: SymbolKind::Function,
    }));
    assert!(tokens.contains(&ql_analysis::SemanticTokenOccurrence {
        span: nth_span(source, "q_add", 2),
        kind: SymbolKind::Function,
    }));
}

#[test]
fn semantic_tokens_follow_top_level_extern_function_surface() {
    let source = r#"
extern "c" fn q_add(left: Int, right: Int) -> Int

fn main() -> Int {
    return q_add(1, 2)
}
"#;

    let analysis = analyzed(source);
    let tokens = analysis.semantic_tokens();

    assert!(tokens.contains(&ql_analysis::SemanticTokenOccurrence {
        span: nth_span(source, "q_add", 1),
        kind: SymbolKind::Function,
    }));
    assert!(tokens.contains(&ql_analysis::SemanticTokenOccurrence {
        span: nth_span(source, "q_add", 2),
        kind: SymbolKind::Function,
    }));
}

#[test]
fn semantic_tokens_follow_top_level_extern_function_definition_surface() {
    let source = r#"
extern "c" fn q_add(left: Int, right: Int) -> Int {
    return left + right
}

fn main() -> Int {
    return q_add(1, 2)
}
"#;

    let analysis = analyzed(source);
    let tokens = analysis.semantic_tokens();

    assert!(tokens.contains(&ql_analysis::SemanticTokenOccurrence {
        span: nth_span(source, "q_add", 1),
        kind: SymbolKind::Function,
    }));
    assert!(tokens.contains(&ql_analysis::SemanticTokenOccurrence {
        span: nth_span(source, "q_add", 2),
        kind: SymbolKind::Function,
    }));
}

#[test]
fn semantic_tokens_follow_free_function_surface() {
    let source = r#"
fn helper(value: Int) -> Int {
    return value
}

fn compute() -> Int {
    return helper(1) + helper(2)
}
"#;

    let analysis = analyzed(source);
    let tokens = analysis.semantic_tokens();

    assert!(tokens.contains(&ql_analysis::SemanticTokenOccurrence {
        span: nth_span(source, "helper", 1),
        kind: SymbolKind::Function,
    }));
    assert!(tokens.contains(&ql_analysis::SemanticTokenOccurrence {
        span: nth_span(source, "helper", 2),
        kind: SymbolKind::Function,
    }));
    assert!(tokens.contains(&ql_analysis::SemanticTokenOccurrence {
        span: nth_span(source, "helper", 3),
        kind: SymbolKind::Function,
    }));
}

#[test]
fn semantic_tokens_follow_same_file_callable_surface() {
    let source = r#"
extern "c" {
    fn q_add(left: Int, right: Int) -> Int
}

extern "c" fn q_sub(left: Int, right: Int) -> Int

extern "c" fn q_mul(left: Int, right: Int) -> Int {
    return left * right
}

fn helper(value: Int) -> Int {
    return value
}

fn compute() -> Int {
    return q_add(1, 2) + q_sub(1, 2) + q_mul(1, 2) + helper(1) + helper(2)
}
"#;

    let analysis = analyzed(source);
    let tokens = analysis.semantic_tokens();
    let cases = [
        ("q_add", vec![1, 2]),
        ("q_sub", vec![1, 2]),
        ("q_mul", vec![1, 2]),
        ("helper", vec![1, 2, 3]),
    ];

    for (name, occurrences) in cases {
        for occurrence in occurrences {
            assert!(
                tokens.contains(&ql_analysis::SemanticTokenOccurrence {
                    span: nth_span(source, name, occurrence),
                    kind: SymbolKind::Function,
                }),
                "{} occurrence {}",
                name,
                occurrence
            );
        }
    }
}
