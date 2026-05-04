use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use ql_analysis::{analyze_package, analyze_package_dependencies, analyze_source};
use ql_lsp::bridge::{
    completion_for_dependency_imports, completion_for_dependency_member_fields,
    completion_for_dependency_methods, completion_for_dependency_struct_fields,
    completion_for_dependency_variants, completion_for_package_analysis,
    declaration_for_dependency_methods, declaration_for_dependency_struct_fields,
    declaration_for_package_analysis, definition_for_dependency_imports,
    definition_for_dependency_methods, definition_for_dependency_struct_fields,
    definition_for_dependency_values, definition_for_dependency_variants,
    definition_for_package_analysis, hover_for_dependency_imports, hover_for_dependency_methods,
    hover_for_dependency_struct_fields, hover_for_dependency_values, hover_for_dependency_variants,
    hover_for_package_analysis, references_for_dependency_imports,
    references_for_dependency_methods, references_for_dependency_struct_fields,
    references_for_dependency_values, references_for_dependency_variants,
    references_for_package_analysis, semantic_tokens_for_dependency_fallback,
    semantic_tokens_for_package_analysis, semantic_tokens_legend, span_to_range,
    type_definition_for_dependency_imports, type_definition_for_dependency_method_types,
    type_definition_for_dependency_struct_field_types, type_definition_for_dependency_values,
};
use ql_span::Span;
use tower_lsp::lsp_types::request::{GotoDeclarationResponse, GotoTypeDefinitionResponse};
use tower_lsp::lsp_types::{
    CompletionItem as LspCompletionItem, CompletionItemKind, CompletionItemTag, CompletionResponse,
    CompletionTextEdit, Documentation, GotoDefinitionResponse, HoverContents, Location, Position,
    SemanticTokenType, SemanticTokensResult, TextEdit, Url,
};

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(prefix: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let path = env::temp_dir().join(format!("{prefix}-{unique}"));
        fs::create_dir_all(&path).expect("create temporary test directory");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn write(&self, relative: &str, contents: &str) -> PathBuf {
        let path = self.path.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent directory for temp file");
        }
        fs::write(&path, contents).expect("write temp file");
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn nth_offset(source: &str, needle: &str, occurrence: usize) -> usize {
    source
        .match_indices(needle)
        .nth(occurrence.saturating_sub(1))
        .map(|(start, _)| start)
        .expect("needle occurrence should exist")
}

fn nth_span(source: &str, needle: &str, occurrence: usize) -> Span {
    source
        .match_indices(needle)
        .nth(occurrence.saturating_sub(1))
        .map(|(start, matched)| Span::new(start, start + matched.len()))
        .expect("needle occurrence should exist")
}

fn offset_to_position(source: &str, offset: usize) -> Position {
    let prefix = &source[..offset];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() as u32;
    let line_start = prefix.rfind('\n').map(|index| index + 1).unwrap_or(0);
    Position::new(line, (prefix[line_start..].chars().count()) as u32)
}

fn completion_item<'a>(items: &'a [LspCompletionItem], label: &str) -> &'a LspCompletionItem {
    items
        .iter()
        .find(|item| item.label == label)
        .unwrap_or_else(|| panic!("completion item `{label}` should exist"))
}

fn completion_documentation_value(item: &LspCompletionItem) -> &str {
    match item
        .documentation
        .as_ref()
        .expect("completion item should have documentation")
    {
        Documentation::String(value) => value,
        Documentation::MarkupContent(markup) => markup.value.as_str(),
    }
}

fn decode_semantic_tokens(
    tokens: &[tower_lsp::lsp_types::SemanticToken],
) -> Vec<(u32, u32, u32, u32)> {
    let mut line = 0u32;
    let mut start = 0u32;
    let mut decoded = Vec::new();

    for token in tokens {
        line += token.delta_line;
        if token.delta_line == 0 {
            start += token.delta_start;
        } else {
            start = token.delta_start;
        }
        decoded.push((line, start, token.length, token.token_type));
    }

    decoded
}

#[test]
fn package_bridge_surfaces_dependency_hover_and_definition() {
    let temp = TempDir::new("ql-lsp-package-bridge");
    let app_root = temp.path().join("workspace").join("app");
    let app_path = temp
        .path()
        .join("workspace")
        .join("app")
        .join("src")
        .join("lib.ql");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    let dep_qi = temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub fn exported(value: Int) -> Int

pub struct Buffer[T] {
    value: T,
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    let source = r#"
package demo.app

use demo.dep.exported as run
use demo.dep.Buffer as Buf

pub fn main(value: Buf[Int]) -> Int {
    return run(1)
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("source should analyze");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");

    let hover = hover_for_package_analysis(
        source,
        &analysis,
        &package,
        offset_to_position(source, nth_offset(source, "Buf", 2)),
    )
    .expect("dependency hover should exist");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(markup.value.contains("**struct** `Buffer`"));
    assert!(markup.value.contains("struct Buffer[T]"));

    let definition = definition_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        offset_to_position(source, nth_offset(source, "run", 2)),
    )
    .expect("dependency definition should exist");
    let GotoDefinitionResponse::Scalar(Location { uri, range }) = definition else {
        panic!("definition should be one location")
    };
    assert_eq!(
        uri.to_file_path()
            .expect("definition URI should convert to a file path")
            .canonicalize()
            .expect("definition path should canonicalize"),
        dep_qi
            .canonicalize()
            .expect("dependency artifact path should canonicalize"),
    );

    let artifact = fs::read_to_string(&dep_qi)
        .expect("dependency interface artifact should exist")
        .replace("\r\n", "\n");
    let snippet = "fn exported(value: Int) -> Int";
    let start = artifact
        .find(snippet)
        .expect("exported signature should exist");
    assert_eq!(
        range,
        span_to_range(&artifact, Span::new(start, start + snippet.len()))
    );
}

#[test]
fn package_bridge_surfaces_grouped_dependency_hover_definition_and_references() {
    let temp = TempDir::new("ql-lsp-grouped-package-bridge");
    let app_root = temp.path().join("workspace").join("app");
    let app_path = temp
        .path()
        .join("workspace")
        .join("app")
        .join("src")
        .join("lib.ql");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    let dep_qi = temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub fn exported(value: Int) -> Int
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    let source = r#"
package demo.app

use demo.dep.{exported as run}

pub fn main() -> Int {
    return run(1)
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("source should analyze");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");

    let hover = hover_for_package_analysis(
        source,
        &analysis,
        &package,
        offset_to_position(source, nth_offset(source, "run", 1)),
    )
    .expect("grouped dependency hover should exist");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(markup.value.contains("**function** `exported`"));
    assert!(markup.value.contains("fn exported(value: Int) -> Int"));

    let definition = definition_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        offset_to_position(source, nth_offset(source, "run", 1)),
    )
    .expect("grouped dependency definition should exist");
    let GotoDefinitionResponse::Scalar(Location { uri, range }) = definition else {
        panic!("definition should be one location")
    };
    assert_eq!(
        uri.to_file_path()
            .expect("definition URI should convert to a file path")
            .canonicalize()
            .expect("definition path should canonicalize"),
        dep_qi
            .canonicalize()
            .expect("dependency artifact path should canonicalize"),
    );

    let artifact = fs::read_to_string(&dep_qi)
        .expect("dependency interface artifact should exist")
        .replace("\r\n", "\n");
    let snippet = "fn exported(value: Int) -> Int";
    let start = artifact
        .find(snippet)
        .expect("exported signature should exist");
    assert_eq!(
        range,
        span_to_range(&artifact, Span::new(start, start + snippet.len()))
    );

    let references = references_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        offset_to_position(source, nth_offset(source, "run", 1)),
        true,
    )
    .expect("grouped dependency references should exist");
    assert_eq!(references.len(), 3);
    assert_eq!(
        references[0]
            .uri
            .to_file_path()
            .expect("definition URI should convert to a file path")
            .canonicalize()
            .expect("definition path should canonicalize"),
        dep_qi
            .canonicalize()
            .expect("dependency artifact path should canonicalize"),
    );
}

#[test]
fn package_bridge_semantic_tokens_cover_dependency_variants_fields_and_methods() {
    let temp = TempDir::new("ql-lsp-package-semantic-tokens");
    let app_root = temp.path().join("workspace").join("app");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub enum Command {
    Retry(Int),
    Stop,
}

pub struct Config {
    value: Int,
    limit: Int,
}

impl Config {
    pub fn ping(self) -> Int
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    let source = r#"
package demo.app

use demo.dep.Command as Cmd
use demo.dep.Config as Cfg

pub fn main(config: Cfg) -> Int {
    let built = Cfg { value: 1, limit: 2 }
    match config {
        Cfg { value: current, limit: 3 } => current,
    }
    let command = Cmd.Retry(1)
    let result = config.ping()
    return built.value
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("source should analyze");
    let SemanticTokensResult::Tokens(tokens) =
        semantic_tokens_for_package_analysis(source, &analysis, &package)
    else {
        panic!("expected full semantic tokens");
    };
    let decoded = decode_semantic_tokens(&tokens.data);
    let legend = semantic_tokens_legend();
    let function_type = legend
        .token_types
        .iter()
        .position(|token_type| *token_type == SemanticTokenType::FUNCTION)
        .expect("function legend entry should exist") as u32;
    let namespace_type = legend
        .token_types
        .iter()
        .position(|token_type| *token_type == SemanticTokenType::NAMESPACE)
        .expect("namespace legend entry should exist") as u32;
    let class_type = legend
        .token_types
        .iter()
        .position(|token_type| *token_type == SemanticTokenType::CLASS)
        .expect("class legend entry should exist") as u32;
    let enum_type = legend
        .token_types
        .iter()
        .position(|token_type| *token_type == SemanticTokenType::ENUM)
        .expect("enum legend entry should exist") as u32;
    let enum_member_type = legend
        .token_types
        .iter()
        .position(|token_type| *token_type == SemanticTokenType::ENUM_MEMBER)
        .expect("enum member legend entry should exist") as u32;
    let property_type = legend
        .token_types
        .iter()
        .position(|token_type| *token_type == SemanticTokenType::PROPERTY)
        .expect("property legend entry should exist") as u32;
    let method_type = legend
        .token_types
        .iter()
        .position(|token_type| *token_type == SemanticTokenType::METHOD)
        .expect("method legend entry should exist") as u32;
    let cfg_import_range = span_to_range(source, nth_span(source, "Cfg", 1));
    let cmd_import_range = span_to_range(source, nth_span(source, "Cmd", 1));

    assert!(decoded.contains(&(
        cfg_import_range.start.line,
        cfg_import_range.start.character,
        cfg_import_range.end.character - cfg_import_range.start.character,
        class_type,
    )));
    assert!(decoded.contains(&(
        cmd_import_range.start.line,
        cmd_import_range.start.character,
        cmd_import_range.end.character - cmd_import_range.start.character,
        enum_type,
    )));
    assert!(!decoded.contains(&(
        cfg_import_range.start.line,
        cfg_import_range.start.character,
        cfg_import_range.end.character - cfg_import_range.start.character,
        namespace_type,
    )));
    assert!(!decoded.contains(&(
        cmd_import_range.start.line,
        cmd_import_range.start.character,
        cmd_import_range.end.character - cmd_import_range.start.character,
        namespace_type,
    )));

    for (span, token_type) in [
        (nth_span(source, "main", 1), function_type),
        (nth_span(source, "Retry", 1), enum_member_type),
        (nth_span(source, "value", 1), property_type),
        (nth_span(source, "value", 2), property_type),
        (nth_span(source, "value", 3), property_type),
        (nth_span(source, "ping", 1), method_type),
    ] {
        let range = span_to_range(source, span);
        assert!(decoded.contains(&(
            range.start.line,
            range.start.character,
            range.end.character - range.start.character,
            token_type,
        )));
    }
}

#[test]
fn package_bridge_keeps_dependency_semantic_tokens_without_semantic_analysis() {
    let temp = TempDir::new("ql-lsp-package-semantic-tokens-broken");
    let app_root = temp.path().join("workspace").join("app");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub enum Command {
    Retry(Int),
    Stop,
}

pub struct Config {
    value: Int,
    limit: Int,
}

impl Config {
    pub fn ping(self) -> Int
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    let source = r#"
package demo.app

use demo.dep.Command as Cmd
use demo.dep.Config as Cfg

pub fn main(config: Cfg) -> Int {
    let built = Cfg { value: 1, limit: 2 }
    let command = Cmd.Retry(1)
    let result = config.ping()
    return "oops" + built.value + result
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");

    let SemanticTokensResult::Tokens(tokens) =
        semantic_tokens_for_dependency_fallback(source, &package)
    else {
        panic!("expected full semantic tokens");
    };
    let decoded = decode_semantic_tokens(&tokens.data);
    let legend = semantic_tokens_legend();
    let enum_member_type = legend
        .token_types
        .iter()
        .position(|token_type| *token_type == SemanticTokenType::ENUM_MEMBER)
        .expect("enum member legend entry should exist") as u32;
    let property_type = legend
        .token_types
        .iter()
        .position(|token_type| *token_type == SemanticTokenType::PROPERTY)
        .expect("property legend entry should exist") as u32;
    let parameter_type = legend
        .token_types
        .iter()
        .position(|token_type| *token_type == SemanticTokenType::PARAMETER)
        .expect("parameter legend entry should exist") as u32;
    let variable_type = legend
        .token_types
        .iter()
        .position(|token_type| *token_type == SemanticTokenType::VARIABLE)
        .expect("variable legend entry should exist") as u32;
    let method_type = legend
        .token_types
        .iter()
        .position(|token_type| *token_type == SemanticTokenType::METHOD)
        .expect("method legend entry should exist") as u32;

    for (span, token_type) in [
        (nth_span(source, "config", 1), parameter_type),
        (nth_span(source, "built", 1), variable_type),
        (nth_span(source, "built", 2), variable_type),
        (nth_span(source, "Retry", 1), enum_member_type),
        (nth_span(source, "value", 1), property_type),
        (nth_span(source, "value", 2), property_type),
        (nth_span(source, "ping", 1), method_type),
    ] {
        let range = span_to_range(source, span);
        assert!(decoded.contains(&(
            range.start.line,
            range.start.character,
            range.end.character - range.start.character,
            token_type,
        )));
    }
}

#[test]
fn package_bridge_keeps_dependency_import_root_semantic_tokens_without_semantic_analysis() {
    let temp = TempDir::new("ql-lsp-package-import-root-semantic-tokens-broken");
    let app_root = temp.path().join("workspace").join("app");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub enum Command {
    Retry(Int),
    Stop,
}

pub struct Config {
    value: Int,
    limit: Int,
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    let source = r#"
package demo.app

use demo.dep.Command as Cmd
use demo.dep.Config as Cfg

pub fn main(config: Cfg) -> Int {
    let built = Cfg { value: 1, limit: 2 }
    let command = Cmd.Retry(1)
    return "oops" + built.value
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");

    let SemanticTokensResult::Tokens(tokens) =
        semantic_tokens_for_dependency_fallback(source, &package)
    else {
        panic!("expected full semantic tokens");
    };
    let decoded = decode_semantic_tokens(&tokens.data);
    let legend = semantic_tokens_legend();
    let class_type = legend
        .token_types
        .iter()
        .position(|token_type| *token_type == SemanticTokenType::CLASS)
        .expect("class legend entry should exist") as u32;
    let enum_type = legend
        .token_types
        .iter()
        .position(|token_type| *token_type == SemanticTokenType::ENUM)
        .expect("enum legend entry should exist") as u32;

    for (span, token_type) in [
        (nth_span(source, "Cfg", 1), class_type),
        (nth_span(source, "Cfg", 2), class_type),
        (nth_span(source, "Cfg", 3), class_type),
        (nth_span(source, "Cmd", 1), enum_type),
        (nth_span(source, "Cmd", 2), enum_type),
    ] {
        let range = span_to_range(source, span);
        assert!(decoded.contains(&(
            range.start.line,
            range.start.character,
            range.end.character - range.start.character,
            token_type,
        )));
    }
}

#[test]
fn package_bridge_keeps_dependency_import_root_semantic_tokens_after_parse_errors() {
    let temp = TempDir::new("ql-lsp-package-import-root-semantic-tokens-parse-errors");
    let app_root = temp.path().join("workspace").join("app");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub enum Command {
    Retry(Int),
    Stop,
}

pub struct Config {
    value: Int,
    limit: Int,
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    let source = r#"
package demo.app

use demo.dep.Command as Cmd
use demo.dep.Config as Cfg

pub fn main(config: Cfg) -> Int {
    let built = Cfg { value: 1, limit: 2
    let command = Cmd.Retry(1)
    return built.value
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_source(source).is_err());
    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");

    let SemanticTokensResult::Tokens(tokens) =
        semantic_tokens_for_dependency_fallback(source, &package)
    else {
        panic!("expected full semantic tokens");
    };
    let decoded = decode_semantic_tokens(&tokens.data);
    let legend = semantic_tokens_legend();
    let class_type = legend
        .token_types
        .iter()
        .position(|token_type| *token_type == SemanticTokenType::CLASS)
        .expect("class legend entry should exist") as u32;
    let enum_type = legend
        .token_types
        .iter()
        .position(|token_type| *token_type == SemanticTokenType::ENUM)
        .expect("enum legend entry should exist") as u32;

    for (span, token_type) in [
        (nth_span(source, "Cfg", 1), class_type),
        (nth_span(source, "Cfg", 2), class_type),
        (nth_span(source, "Cfg", 3), class_type),
        (nth_span(source, "Cmd", 1), enum_type),
        (nth_span(source, "Cmd", 2), enum_type),
    ] {
        let range = span_to_range(source, span);
        assert!(decoded.contains(&(
            range.start.line,
            range.start.character,
            range.end.character - range.start.character,
            token_type,
        )));
    }
}

#[test]
fn package_bridge_surfaces_dependency_import_completion() {
    let temp = TempDir::new("ql-lsp-package-completion");
    let app_root = temp.path().join("workspace").join("app");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Buffer[T] {
    value: T,
}
pub const DEFAULT_PORT: Int
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    let source = r#"
package demo.app

use demo.dep.Bu

pub fn main() -> Int {
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("source should analyze");

    let Some(CompletionResponse::Array(items)) = completion_for_package_analysis(
        source,
        &analysis,
        &package,
        offset_to_position(source, nth_offset(source, "Bu", 1) + "Bu".len()),
    ) else {
        panic!("dependency completion should exist")
    };

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "Buffer");
    assert_eq!(items[0].kind, Some(CompletionItemKind::STRUCT));
    assert!(
        items[0]
            .detail
            .as_deref()
            .is_some_and(|detail| detail.starts_with("struct Buffer[T] {"))
    );
    assert_eq!(
        items[0].text_edit,
        Some(CompletionTextEdit::Edit(TextEdit::new(
            span_to_range(
                source,
                Span::new(
                    nth_offset(source, "Bu", 1),
                    nth_offset(source, "Bu", 1) + "Bu".len(),
                ),
            ),
            "Buffer".to_owned(),
        ))),
    );
}

#[test]
#[allow(deprecated)]
fn package_bridge_marks_stdlib_compat_import_completions_deprecated() {
    let temp = TempDir::new("ql-lsp-stdlib-compat-completion");
    let app_root = temp.path().join("workspace").join("app");

    temp.write(
        "workspace/option/qlang.toml",
        r#"
[package]
name = "std.option"
"#,
    );
    temp.write(
        "workspace/option/std.option.qi",
        r#"
// qlang interface v1
// package: std.option

// source: src/lib.ql
package std.option

pub enum Option[T] {
    Some(T),
    None,
}
pub enum IntOption {
    Some(Int),
    None,
}
pub fn some[T](value: T) -> Option[T]
pub fn some_int(value: Int) -> IntOption
"#,
    );
    temp.write(
        "workspace/result/qlang.toml",
        r#"
[package]
name = "std.result"
"#,
    );
    temp.write(
        "workspace/result/std.result.qi",
        r#"
// qlang interface v1
// package: std.result

// source: src/lib.ql
package std.result

pub enum Result[T, E] {
    Ok(T),
    Err(E),
}
pub enum IntResult {
    Ok(Int),
    Err(Int),
}
pub fn ok[T, E](value: T) -> Result[T, E]
pub fn ok_int(value: Int) -> IntResult
"#,
    );
    temp.write(
        "workspace/array/qlang.toml",
        r#"
[package]
name = "std.array"
"#,
    );
    temp.write(
        "workspace/array/std.array.qi",
        r#"
// qlang interface v1
// package: std.array

// source: src/lib.ql
package std.array

pub fn sum_int_array[N](values: [Int; N]) -> Int
pub fn sum3_int_array(values: [Int; 3]) -> Int
pub fn repeat3_array[T](value: T) -> [T; 3]
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../option", "../result", "../array"]
"#,
    );
    let source = r#"
package demo.app

use std.option.
use std.result.
use std.array.

pub fn main() -> Int {
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");

    let Some(CompletionResponse::Array(option_items)) = completion_for_dependency_imports(
        source,
        &package,
        offset_to_position(
            source,
            nth_offset(source, "std.option.", 1) + "std.option.".len(),
        ),
    ) else {
        panic!("std.option completion should exist")
    };
    assert_stdlib_recommended_completion(completion_item(&option_items, "Option"));
    assert_stdlib_recommended_completion(completion_item(&option_items, "some"));
    assert_stdlib_compat_completion(completion_item(&option_items, "IntOption"));
    assert_stdlib_compat_completion(completion_item(&option_items, "some_int"));

    let Some(CompletionResponse::Array(result_items)) = completion_for_dependency_imports(
        source,
        &package,
        offset_to_position(
            source,
            nth_offset(source, "std.result.", 1) + "std.result.".len(),
        ),
    ) else {
        panic!("std.result completion should exist")
    };
    assert_stdlib_recommended_completion(completion_item(&result_items, "Result"));
    assert_stdlib_recommended_completion(completion_item(&result_items, "ok"));
    assert_stdlib_compat_completion(completion_item(&result_items, "IntResult"));
    assert_stdlib_compat_completion(completion_item(&result_items, "ok_int"));

    let Some(CompletionResponse::Array(array_items)) = completion_for_dependency_imports(
        source,
        &package,
        offset_to_position(
            source,
            nth_offset(source, "std.array.", 1) + "std.array.".len(),
        ),
    ) else {
        panic!("std.array completion should exist")
    };
    assert_stdlib_recommended_completion(completion_item(&array_items, "sum_int_array"));
    assert_stdlib_compat_completion(completion_item(&array_items, "sum3_int_array"));
    assert_stdlib_compat_completion(completion_item(&array_items, "repeat3_array"));
}

#[allow(deprecated)]
fn assert_stdlib_recommended_completion(item: &LspCompletionItem) {
    assert_eq!(item.tags, None);
    assert_eq!(item.deprecated, None);
    assert_eq!(item.sort_text, None);
}

#[allow(deprecated)]
fn assert_stdlib_compat_completion(item: &LspCompletionItem) {
    assert_eq!(item.tags, Some(vec![CompletionItemTag::DEPRECATED]));
    assert_eq!(item.deprecated, Some(true));
    assert!(
        item.sort_text
            .as_deref()
            .is_some_and(|text| text.starts_with("zz_"))
    );
    assert!(
        completion_documentation_value(item).contains("Compatibility API"),
        "compatibility completion should document the preferred API"
    );
}

#[test]
fn package_bridge_surfaces_dependency_import_path_segment_completion() {
    let temp = TempDir::new("ql-lsp-package-path-completion");
    let app_root = temp.path().join("workspace").join("app");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Buffer[T] {
    value: T,
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    let source = r#"
package demo.app

use demo.d

pub fn main() -> Int {
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("source should analyze");
    let completion_offset = nth_offset(source, "demo.d", 1) + "demo.d".len();

    let Some(CompletionResponse::Array(items)) = completion_for_package_analysis(
        source,
        &analysis,
        &package,
        offset_to_position(source, completion_offset),
    ) else {
        panic!("dependency path completion should exist")
    };

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "dep");
    assert_eq!(items[0].kind, Some(CompletionItemKind::MODULE));
    assert_eq!(items[0].detail.as_deref(), Some("package demo.dep"));
    assert_eq!(
        items[0].text_edit,
        Some(CompletionTextEdit::Edit(TextEdit::new(
            span_to_range(
                source,
                Span::new(
                    nth_offset(source, "demo.d", 1) + "demo.".len(),
                    nth_offset(source, "demo.d", 1) + "demo.d".len(),
                ),
            ),
            "dep".to_owned(),
        ))),
    );
}

#[test]
fn package_bridge_surfaces_grouped_dependency_import_completion() {
    let temp = TempDir::new("ql-lsp-grouped-package-completion");
    let app_root = temp.path().join("workspace").join("app");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub fn exported(value: Int) -> Int
pub struct Buffer[T] {
    value: T,
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    let source = r#"
package demo.app

use demo.dep.{exported as run, Bu}

pub fn main() -> Int {
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("source should analyze");

    let Some(CompletionResponse::Array(items)) = completion_for_package_analysis(
        source,
        &analysis,
        &package,
        offset_to_position(source, nth_offset(source, "Bu", 1) + "Bu".len()),
    ) else {
        panic!("grouped dependency completion should exist")
    };

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "Buffer");
    assert_eq!(items[0].kind, Some(CompletionItemKind::STRUCT));
    assert_eq!(
        items[0].text_edit,
        Some(CompletionTextEdit::Edit(TextEdit::new(
            span_to_range(
                source,
                Span::new(
                    nth_offset(source, "Bu", 1),
                    nth_offset(source, "Bu", 1) + "Bu".len(),
                ),
            ),
            "Buffer".to_owned(),
        ))),
    );
}

#[test]
fn package_bridge_grouped_dependency_completion_skips_existing_items() {
    let temp = TempDir::new("ql-lsp-grouped-package-dedup");
    let app_root = temp.path().join("workspace").join("app");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub fn exported(value: Int) -> Int
pub struct Buffer[T] {
    value: T,
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    let source = r#"
package demo.app

use demo.dep.{exported, }

pub fn main() -> Int {
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("source should analyze");

    let Some(CompletionResponse::Array(items)) = completion_for_package_analysis(
        source,
        &analysis,
        &package,
        offset_to_position(source, nth_offset(source, ", }", 1) + 2),
    ) else {
        panic!("grouped dependency completion should exist")
    };

    assert!(items.iter().any(|item| item.label == "Buffer"));
    assert!(!items.iter().any(|item| item.label == "exported"));
}

#[test]
fn package_bridge_surfaces_dependency_import_completion_without_semantic_analysis() {
    let temp = TempDir::new("ql-lsp-package-broken-completion");
    let app_root = temp.path().join("workspace").join("app");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Buffer[T] {
    value: T,
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    let source = r#"
package demo.app

use demo.dep.Bu

pub fn main( -> Int {
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");
    assert!(analyze_source(source).is_err());

    let Some(CompletionResponse::Array(items)) = completion_for_dependency_imports(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "Bu", 1) + "Bu".len()),
    ) else {
        panic!("dependency completion should exist even without semantic analysis")
    };

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "Buffer");
    assert_eq!(items[0].kind, Some(CompletionItemKind::STRUCT));
}

#[test]
fn package_bridge_surfaces_dependency_variant_completion_through_import_alias() {
    let temp = TempDir::new("ql-lsp-package-variant-completion");
    let app_root = temp.path().join("workspace").join("app");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub enum Command {
    Retry(Int),
    Stop,
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    let source = r#"
package demo.app

use demo.dep.Command as Cmd

pub fn main() -> Int {
    return Cmd.Re()
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("source should analyze");

    let Some(CompletionResponse::Array(items)) = completion_for_package_analysis(
        source,
        &analysis,
        &package,
        offset_to_position(source, nth_offset(source, ".Re", 1) + 3),
    ) else {
        panic!("dependency variant completion should exist")
    };

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "Retry");
    assert!(
        items
            .iter()
            .all(|item| item.kind == Some(CompletionItemKind::ENUM_MEMBER))
    );
    assert!(items.iter().any(|item| {
        item.label == "Retry" && item.detail.as_deref() == Some("variant Command.Retry(Int)")
    }));
}

#[test]
fn package_bridge_surfaces_dependency_variant_completion_without_semantic_analysis() {
    let temp = TempDir::new("ql-lsp-package-variant-broken-completion");
    let app_root = temp.path().join("workspace").join("app");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub enum Command {
    Retry(Int),
    Stop,
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    let source = r#"
package demo.app

use demo.dep.Command as Cmd

pub fn main(flag: Bool) -> Int {
    if flag {
        return Cmd.Re()
    }
    return "oops"
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");

    let Some(CompletionResponse::Array(items)) = completion_for_dependency_variants(
        source,
        &package,
        offset_to_position(source, nth_offset(source, ".Re", 1) + 3),
    ) else {
        panic!("dependency variant completion should exist even without semantic analysis")
    };

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "Retry");
    assert_eq!(items[0].kind, Some(CompletionItemKind::ENUM_MEMBER));
    assert_eq!(
        items[0].detail.as_deref(),
        Some("variant Command.Retry(Int)")
    );
}

#[test]
fn package_bridge_surfaces_dependency_struct_field_completion_without_semantic_analysis() {
    let temp = TempDir::new("ql-lsp-package-struct-field-completion");
    let app_root = temp.path().join("workspace").join("app");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Config {
    value: Int,
    flag: Bool,
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    let source = r#"
package demo.app

use demo.dep.Config as Cfg

pub fn main(current: Int, built: Cfg) -> Int {
    let next = Cfg { value: current, fl: true }
    let Cfg { value: reused, fl: enabled } = built
    return missing
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");

    let Some(CompletionResponse::Array(items)) = completion_for_dependency_struct_fields(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "fl", 1) + "fl".len()),
    ) else {
        panic!("struct field completion should exist even without semantic analysis")
    };

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "flag");
    assert_eq!(items[0].kind, Some(CompletionItemKind::FIELD));
    assert_eq!(items[0].detail.as_deref(), Some("field flag: Bool"));
    assert_eq!(
        items[0].text_edit,
        Some(CompletionTextEdit::Edit(TextEdit::new(
            span_to_range(
                source,
                Span::new(
                    nth_offset(source, "fl", 1),
                    nth_offset(source, "fl", 1) + "fl".len(),
                ),
            ),
            "flag".to_owned(),
        ))),
    );
}

#[test]
fn package_bridge_surfaces_dependency_variant_hover_and_definition() {
    let temp = TempDir::new("ql-lsp-package-variant-query");
    let app_root = temp.path().join("workspace").join("app");
    let app_path = temp
        .path()
        .join("workspace")
        .join("app")
        .join("src")
        .join("lib.ql");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    let dep_qi = temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub enum Command {
    Retry(Int),
    Stop,
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    let source = r#"
package demo.app

use demo.dep.Command as Cmd

pub fn main() -> Int {
    let value = Cmd.Retry(1)
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("source should analyze");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");

    let hover = hover_for_package_analysis(
        source,
        &analysis,
        &package,
        offset_to_position(source, nth_offset(source, "Retry", 1)),
    )
    .expect("dependency variant hover should exist");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(markup.value.contains("**variant** `Retry`"));
    assert!(markup.value.contains("variant Command.Retry(Int)"));

    let definition = definition_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        offset_to_position(source, nth_offset(source, "Retry", 1)),
    )
    .expect("dependency variant definition should exist");
    let GotoDefinitionResponse::Scalar(Location { uri, range }) = definition else {
        panic!("definition should be one location")
    };
    assert_eq!(
        uri.to_file_path()
            .expect("definition URI should convert to a file path")
            .canonicalize()
            .expect("definition path should canonicalize"),
        dep_qi
            .canonicalize()
            .expect("dependency artifact path should canonicalize"),
    );

    let artifact = fs::read_to_string(&dep_qi)
        .expect("dependency interface artifact should exist")
        .replace("\r\n", "\n");
    let snippet = "Retry";
    let start = artifact
        .find(snippet)
        .expect("variant name should exist in dependency artifact");
    assert_eq!(
        range,
        span_to_range(&artifact, Span::new(start, start + snippet.len()))
    );
}

#[test]
fn package_bridge_surfaces_dependency_variant_hover_and_definition_without_semantic_analysis() {
    let temp = TempDir::new("ql-lsp-package-variant-broken-query");
    let app_root = temp.path().join("workspace").join("app");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    let dep_qi = temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub enum Command {
    Retry(Int),
    Stop,
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    let source = r#"
package demo.app

use demo.dep.Command as Cmd

pub fn main(flag: Bool) -> Int {
    let value = Cmd.Retry(1)
    if flag {
        return 0
    }
    return "oops"
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");

    let hover = hover_for_dependency_variants(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "Retry", 1)),
    )
    .expect("dependency variant hover should exist even without semantic analysis");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(markup.value.contains("**variant** `Retry`"));
    assert!(markup.value.contains("variant Command.Retry(Int)"));

    let definition = definition_for_dependency_variants(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "Retry", 1)),
    )
    .expect("dependency variant definition should exist even without semantic analysis");
    let GotoDefinitionResponse::Scalar(Location {
        uri: definition_uri,
        range,
    }) = definition
    else {
        panic!("definition should be one location")
    };
    assert_eq!(
        definition_uri
            .to_file_path()
            .expect("definition URI should convert to a file path")
            .canonicalize()
            .expect("definition path should canonicalize"),
        dep_qi
            .canonicalize()
            .expect("dependency artifact path should canonicalize"),
    );

    let artifact = fs::read_to_string(&dep_qi)
        .expect("dependency interface artifact should exist")
        .replace("\r\n", "\n");
    let snippet = "Retry";
    let start = artifact
        .find(snippet)
        .expect("variant name should exist in dependency artifact");
    assert_eq!(
        range,
        span_to_range(&artifact, Span::new(start, start + snippet.len()))
    );
}

#[test]
fn package_bridge_surfaces_dependency_variant_references() {
    let temp = TempDir::new("ql-lsp-package-variant-references");
    let app_root = temp.path().join("workspace").join("app");
    let app_path = temp
        .path()
        .join("workspace")
        .join("app")
        .join("src")
        .join("lib.ql");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    let dep_qi = temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub enum Command {
    Retry(Int),
    Stop,
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    let source = r#"
package demo.app

use demo.dep.Command as Cmd

pub fn first() -> Int {
    let value = Cmd.Retry(1)
    return 0
}

pub fn second() -> Int {
    let value = Cmd.Retry(2)
    return 1
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("source should analyze");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");

    let with_declaration = references_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        offset_to_position(source, nth_offset(source, "Retry", 2)),
        true,
    )
    .expect("dependency variant references should exist");
    assert_eq!(with_declaration.len(), 3);
    assert_eq!(
        with_declaration[0]
            .uri
            .to_file_path()
            .expect("definition URI should convert to a file path")
            .canonicalize()
            .expect("definition path should canonicalize"),
        dep_qi
            .canonicalize()
            .expect("dependency artifact path should canonicalize"),
    );

    let artifact = fs::read_to_string(&dep_qi)
        .expect("dependency interface artifact should exist")
        .replace("\r\n", "\n");
    let snippet = "Retry";
    let start = artifact
        .find(snippet)
        .expect("variant name should exist in dependency artifact");
    assert_eq!(
        with_declaration[0].range,
        span_to_range(&artifact, Span::new(start, start + snippet.len()))
    );

    let without_declaration = references_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        offset_to_position(source, nth_offset(source, "Retry", 2)),
        false,
    )
    .expect("dependency variant references should exist without declaration");
    assert_eq!(without_declaration.len(), 2);
    assert!(
        without_declaration
            .iter()
            .all(|location| location.uri == uri)
    );
    assert_eq!(
        without_declaration[0].range,
        span_to_range(
            source,
            Span::new(
                nth_offset(source, "Retry", 1),
                nth_offset(source, "Retry", 1) + "Retry".len(),
            ),
        )
    );
    assert_eq!(
        without_declaration[1].range,
        span_to_range(
            source,
            Span::new(
                nth_offset(source, "Retry", 2),
                nth_offset(source, "Retry", 2) + "Retry".len(),
            ),
        )
    );
}

#[test]
fn package_bridge_surfaces_dependency_struct_field_queries() {
    let temp = TempDir::new("ql-lsp-package-struct-field-queries");
    let app_root = temp.path().join("workspace").join("app");
    let app_path = temp
        .path()
        .join("workspace")
        .join("app")
        .join("src")
        .join("lib.ql");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    let dep_qi = temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Config {
    value: Int,
    limit: Int,
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    let source = r#"
package demo.app

use demo.dep.Config as Cfg

pub fn read(config: Cfg) -> Int {
    let built = Cfg { value: 1, limit: 2 }
    match config {
        Cfg { value: current, limit: 3 } => current,
    }
    return built.value
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("source should analyze");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");
    let literal_field = nth_offset(source, "value", 1);
    let pattern_field = nth_offset(source, "value", 2);

    let hover = hover_for_package_analysis(
        source,
        &analysis,
        &package,
        offset_to_position(source, literal_field),
    )
    .expect("dependency struct field hover should exist");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(markup.value.contains("**field** `value`"));
    assert!(markup.value.contains("field value: Int"));

    let definition = definition_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        offset_to_position(source, pattern_field),
    )
    .expect("dependency struct field definition should exist");
    let GotoDefinitionResponse::Scalar(Location { uri, range }) = definition else {
        panic!("definition should be one location")
    };
    assert_eq!(
        uri.to_file_path()
            .expect("definition URI should convert to a file path")
            .canonicalize()
            .expect("definition path should canonicalize"),
        dep_qi
            .canonicalize()
            .expect("dependency artifact path should canonicalize"),
    );
    let artifact = fs::read_to_string(&dep_qi)
        .expect("dependency interface artifact should exist")
        .replace("\r\n", "\n");
    let snippet = "value";
    let start = artifact
        .find(snippet)
        .expect("field name should exist in dependency artifact");
    assert_eq!(
        range,
        span_to_range(&artifact, Span::new(start, start + snippet.len()))
    );

    let with_declaration = references_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        offset_to_position(source, literal_field),
        true,
    )
    .expect("dependency struct field references should exist");
    let member_field = nth_offset(source, "value", 3);
    assert_eq!(with_declaration.len(), 4);
    assert_eq!(
        with_declaration[0]
            .uri
            .to_file_path()
            .expect("definition URI should convert to a file path")
            .canonicalize()
            .expect("definition path should canonicalize"),
        dep_qi
            .canonicalize()
            .expect("dependency artifact path should canonicalize"),
    );
    assert_eq!(
        with_declaration[1].range,
        span_to_range(
            source,
            Span::new(literal_field, literal_field + "value".len())
        )
    );
    assert_eq!(
        with_declaration[2].range,
        span_to_range(
            source,
            Span::new(pattern_field, pattern_field + "value".len())
        )
    );
    assert_eq!(
        with_declaration[3].range,
        span_to_range(
            source,
            Span::new(member_field, member_field + "value".len())
        )
    );

    let without_declaration = references_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        offset_to_position(source, literal_field),
        false,
    )
    .expect("dependency struct field references should exist without declaration");
    assert_eq!(without_declaration.len(), 3);
    assert!(
        without_declaration
            .iter()
            .all(|location| location.uri == uri)
    );
}

#[test]
fn package_bridge_surfaces_dependency_struct_member_field_queries() {
    let temp = TempDir::new("ql-lsp-package-struct-member-query");
    let app_root = temp.path().join("workspace").join("app");
    let app_path = temp
        .path()
        .join("workspace")
        .join("app")
        .join("src")
        .join("lib.ql");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    let dep_qi = temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Config {
    value: Int,
    limit: Int,
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    let source = r#"
package demo.app

use demo.dep.Config as Cfg

pub fn read(config: Cfg) -> Int {
    let built = Cfg { value: 1, limit: 2 }
    return config.value + built.value
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("source should analyze");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");
    let config_member = nth_offset(source, "value", 2);
    let built_member = nth_offset(source, "value", 3);

    let hover = hover_for_package_analysis(
        source,
        &analysis,
        &package,
        offset_to_position(source, built_member),
    )
    .expect("dependency struct member hover should exist");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(markup.value.contains("**field** `value`"));
    assert!(markup.value.contains("field value: Int"));

    let definition = definition_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        offset_to_position(source, config_member),
    )
    .expect("dependency struct member definition should exist");
    let GotoDefinitionResponse::Scalar(Location {
        uri: definition_uri,
        range,
    }) = definition
    else {
        panic!("definition should be one location")
    };
    assert_eq!(
        definition_uri
            .to_file_path()
            .expect("definition URI should convert to a file path")
            .canonicalize()
            .expect("definition path should canonicalize"),
        dep_qi
            .canonicalize()
            .expect("dependency artifact path should canonicalize"),
    );
    let artifact = fs::read_to_string(&dep_qi)
        .expect("dependency interface artifact should exist")
        .replace("\r\n", "\n");
    let snippet = "value";
    let start = artifact
        .find(snippet)
        .expect("field name should exist in dependency artifact");
    assert_eq!(
        range,
        span_to_range(&artifact, Span::new(start, start + snippet.len()))
    );

    let with_declaration = references_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        offset_to_position(source, built_member),
        true,
    )
    .expect("dependency struct member references should exist");
    assert_eq!(with_declaration.len(), 4);
    assert_eq!(
        with_declaration[0]
            .uri
            .to_file_path()
            .expect("definition URI should convert to a file path")
            .canonicalize()
            .expect("definition path should canonicalize"),
        dep_qi
            .canonicalize()
            .expect("dependency artifact path should canonicalize"),
    );
    let local_ranges = with_declaration[1..]
        .iter()
        .map(|location| location.range)
        .collect::<Vec<_>>();
    assert!(local_ranges.contains(&span_to_range(
        source,
        Span::new(config_member, config_member + "value".len())
    )));
    assert!(local_ranges.contains(&span_to_range(
        source,
        Span::new(built_member, built_member + "value".len())
    )));

    let without_declaration = references_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        offset_to_position(source, config_member),
        false,
    )
    .expect("dependency struct member references should exist without declaration");
    assert_eq!(without_declaration.len(), 3);
    assert!(
        without_declaration
            .iter()
            .all(|location| location.uri == uri)
    );
}

#[test]
fn package_bridge_surfaces_dependency_struct_member_method_queries() {
    let temp = TempDir::new("ql-lsp-package-struct-member-method-query");
    let app_root = temp.path().join("workspace").join("app");
    let app_path = temp
        .path()
        .join("workspace")
        .join("app")
        .join("src")
        .join("lib.ql");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    let dep_qi = temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Config {
    value: Int,
}

impl Config {
    pub fn get(self) -> Int
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    let source = r#"
package demo.app

use demo.dep.Config as Cfg

pub fn read(config: Cfg) -> Int {
    let built = Cfg { value: 1 }
    return config.get() + built.get()
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("source should analyze");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");
    let config_method = nth_offset(source, "get", 1);
    let built_method = nth_offset(source, "get", 2);

    let hover = hover_for_package_analysis(
        source,
        &analysis,
        &package,
        offset_to_position(source, config_method),
    )
    .expect("dependency struct member method hover should exist");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(markup.value.contains("**method** `get`"));
    assert!(markup.value.contains("fn get(self) -> Int"));

    let definition = definition_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        offset_to_position(source, built_method),
    )
    .expect("dependency struct member method definition should exist");
    let GotoDefinitionResponse::Scalar(Location {
        uri: definition_uri,
        range,
    }) = definition
    else {
        panic!("definition should be one location")
    };
    assert_eq!(
        definition_uri
            .to_file_path()
            .expect("definition URI should convert to a file path")
            .canonicalize()
            .expect("definition path should canonicalize"),
        dep_qi
            .canonicalize()
            .expect("dependency artifact path should canonicalize"),
    );
    let artifact = fs::read_to_string(&dep_qi)
        .expect("dependency interface artifact should exist")
        .replace("\r\n", "\n");
    let snippet = "pub fn get(self) -> Int";
    let start = artifact
        .find(snippet)
        .map(|offset| offset + "pub fn ".len())
        .expect("method signature should exist in dependency artifact");
    assert_eq!(
        range,
        span_to_range(&artifact, Span::new(start, start + "get".len()))
    );

    let with_declaration = references_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        offset_to_position(source, config_method),
        true,
    )
    .expect("dependency struct member method references should exist");
    assert_eq!(with_declaration.len(), 3);
    assert_eq!(
        with_declaration[0]
            .uri
            .to_file_path()
            .expect("definition URI should convert to a file path")
            .canonicalize()
            .expect("definition path should canonicalize"),
        dep_qi
            .canonicalize()
            .expect("dependency artifact path should canonicalize"),
    );
    assert!(
        with_declaration[1..]
            .iter()
            .all(|location| location.uri == uri)
    );

    let without_declaration = references_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        offset_to_position(source, built_method),
        false,
    )
    .expect("dependency struct member method references should exist without declaration");
    assert_eq!(without_declaration.len(), 2);
    assert!(
        without_declaration
            .iter()
            .all(|location| location.uri == uri)
    );
}

#[test]
fn package_bridge_surfaces_dependency_struct_field_hover_and_definition_without_semantic_analysis()
{
    let temp = TempDir::new("ql-lsp-package-struct-field-broken-query");
    let app_root = temp.path().join("workspace").join("app");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    let dep_qi = temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Config {
    value: Int,
    limit: Int,
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    let source = r#"
package demo.app

use demo.dep.Config as Cfg

pub fn read(config: Cfg) -> Int {
    let built = Cfg { value: 1, limit: 2 }
    match config {
        Cfg { value: current, limit: 3 } => current,
    }
    return "oops"
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");
    let literal_field = nth_offset(source, "value", 1);
    let pattern_field = nth_offset(source, "value", 2);

    let hover = hover_for_dependency_struct_fields(
        source,
        &package,
        offset_to_position(source, literal_field),
    )
    .expect("dependency struct field hover should exist even without semantic analysis");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(markup.value.contains("**field** `value`"));
    assert!(markup.value.contains("field value: Int"));

    let definition = definition_for_dependency_struct_fields(
        source,
        &package,
        offset_to_position(source, pattern_field),
    )
    .expect("dependency struct field definition should exist even without semantic analysis");
    let GotoDefinitionResponse::Scalar(Location {
        uri: definition_uri,
        range,
    }) = definition
    else {
        panic!("definition should be one location")
    };
    assert_eq!(
        definition_uri
            .to_file_path()
            .expect("definition URI should convert to a file path")
            .canonicalize()
            .expect("definition path should canonicalize"),
        dep_qi
            .canonicalize()
            .expect("dependency artifact path should canonicalize"),
    );
    let artifact = fs::read_to_string(&dep_qi)
        .expect("dependency interface artifact should exist")
        .replace("\r\n", "\n");
    let snippet = "value";
    let start = artifact
        .find(snippet)
        .expect("field name should exist in dependency artifact");
    assert_eq!(
        range,
        span_to_range(&artifact, Span::new(start, start + snippet.len()))
    );
}

#[test]
fn package_bridge_surfaces_dependency_struct_member_method_queries_without_semantic_analysis() {
    let temp = TempDir::new("ql-lsp-package-struct-member-method-broken-query");
    let app_root = temp.path().join("workspace").join("app");
    let app_path = temp
        .path()
        .join("workspace")
        .join("app")
        .join("src")
        .join("lib.ql");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    let dep_qi = temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Config {
    value: Int,
}

impl Config {
    pub fn get(self) -> Int
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    let source = r#"
package demo.app

use demo.dep.Config as Cfg

pub fn read(config: Cfg) -> Int {
    let built = Cfg { value: 1 }
    let broken: Int = "oops"
    return config.get() + built.get()
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");
    let config_method = nth_offset(source, "get", 1);
    let built_method = nth_offset(source, "get", 2);

    let hover =
        hover_for_dependency_methods(source, &package, offset_to_position(source, config_method))
            .expect(
                "dependency struct member method hover should exist even without semantic analysis",
            );
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(markup.value.contains("**method** `get`"));
    assert!(markup.value.contains("fn get(self) -> Int"));

    let definition = definition_for_dependency_methods(
        source,
        &package,
        offset_to_position(source, built_method),
    )
    .expect(
        "dependency struct member method definition should exist even without semantic analysis",
    );
    let GotoDefinitionResponse::Scalar(Location {
        uri: definition_uri,
        range,
    }) = definition
    else {
        panic!("definition should be one location")
    };
    assert_eq!(
        definition_uri
            .to_file_path()
            .expect("definition URI should convert to a file path")
            .canonicalize()
            .expect("definition path should canonicalize"),
        dep_qi
            .canonicalize()
            .expect("dependency artifact path should canonicalize"),
    );
    let artifact = fs::read_to_string(&dep_qi)
        .expect("dependency interface artifact should exist")
        .replace("\r\n", "\n");
    let snippet = "pub fn get(self) -> Int";
    let start = artifact
        .find(snippet)
        .map(|offset| offset + "pub fn ".len())
        .expect("method signature should exist in dependency artifact");
    assert_eq!(
        range,
        span_to_range(&artifact, Span::new(start, start + "get".len()))
    );

    let declaration = declaration_for_dependency_methods(
        source,
        &package,
        offset_to_position(source, config_method),
    )
    .expect(
        "dependency struct member method declaration should exist even without semantic analysis",
    );
    let GotoDeclarationResponse::Scalar(Location {
        uri: declaration_uri,
        range: declaration_range,
    }) = declaration
    else {
        panic!("declaration should be one location")
    };
    assert_eq!(
        declaration_uri
            .to_file_path()
            .expect("declaration URI should convert to a file path")
            .canonicalize()
            .expect("declaration path should canonicalize"),
        dep_qi
            .canonicalize()
            .expect("dependency artifact path should canonicalize"),
    );
    assert_eq!(declaration_range, range);

    let with_declaration = references_for_dependency_methods(
        &uri,
        source,
        &package,
        offset_to_position(source, config_method),
        true,
    )
    .expect(
        "dependency struct member method references should exist even without semantic analysis",
    );
    assert_eq!(with_declaration.len(), 3);
    assert_eq!(
        with_declaration[0]
            .uri
            .to_file_path()
            .expect("definition URI should convert to a file path")
            .canonicalize()
            .expect("definition path should canonicalize"),
        dep_qi
            .canonicalize()
            .expect("dependency artifact path should canonicalize"),
    );
    assert!(
        with_declaration[1..]
            .iter()
            .all(|location| location.uri == uri)
    );

    let without_declaration = references_for_dependency_methods(
        &uri,
        source,
        &package,
        offset_to_position(source, built_method),
        false,
    )
    .expect("dependency struct member method references should exist without semantic analysis");
    assert_eq!(without_declaration.len(), 2);
    assert!(
        without_declaration
            .iter()
            .all(|location| location.uri == uri)
    );
}

#[test]
fn package_bridge_completes_dependency_struct_member_methods() {
    let temp = TempDir::new("ql-lsp-package-struct-member-method-completion");
    let app_root = temp.path().join("workspace").join("app");
    let source = r#"
package demo.app

use demo.dep.Config as Cfg

pub fn read(config: Cfg) -> Int {
    let built = Cfg { value: 1 }
    return config.get() + built.ge
}
"#;
    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Config {
    value: Int,
}

impl Config {
    pub fn get(self) -> Int
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");
    let analysis = analyze_source(source).expect("analysis should succeed for completion query");

    let Some(CompletionResponse::Array(items)) = completion_for_package_analysis(
        source,
        &analysis,
        &package,
        offset_to_position(source, nth_offset(source, ".ge", 1) + ".ge".len()),
    ) else {
        panic!("dependency struct member method completion should exist");
    };
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "get");
    assert_eq!(items[0].kind, Some(CompletionItemKind::METHOD));
    assert_eq!(items[0].detail.as_deref(), Some("fn get(self) -> Int"));
}

#[test]
fn package_bridge_completes_dependency_struct_member_methods_without_semantic_analysis() {
    let temp = TempDir::new("ql-lsp-package-struct-member-method-broken-completion");
    let app_root = temp.path().join("workspace").join("app");
    let source = r#"
package demo.app

use demo.dep.Config as Cfg

pub fn read(config: Cfg) -> Int {
    let built = Cfg { value: 1 }
    let broken: Int = "oops"
    return config.ge + built.ge
}
"#;
    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Config {
    value: Int,
}

impl Config {
    pub fn get(self) -> Int
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");

    let Some(CompletionResponse::Array(items)) = completion_for_dependency_methods(
        source,
        &package,
        offset_to_position(source, nth_offset(source, ".ge", 1) + ".ge".len()),
    ) else {
        panic!("dependency struct member method completion should exist without semantic analysis");
    };
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "get");
    assert_eq!(items[0].kind, Some(CompletionItemKind::METHOD));
    assert_eq!(items[0].detail.as_deref(), Some("fn get(self) -> Int"));
}

#[test]
fn package_bridge_completes_dependency_struct_member_fields() {
    let temp = TempDir::new("ql-lsp-package-struct-member-field-completion");
    let app_root = temp.path().join("workspace").join("app");
    let source = r#"
package demo.app

use demo.dep.Config as Cfg

pub fn read(config: Cfg) -> Int {
    let built = Cfg { value: 1 }
    return config.va + built.va
}
"#;
    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Config {
    value: Int,
}

impl Config {
    pub fn get(self) -> Int
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");
    let analysis = analyze_source(source).expect("analysis should succeed for completion query");

    let Some(CompletionResponse::Array(items)) = completion_for_package_analysis(
        source,
        &analysis,
        &package,
        offset_to_position(source, nth_offset(source, ".va", 1) + ".va".len()),
    ) else {
        panic!("dependency struct member field completion should exist");
    };
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "value");
    assert_eq!(items[0].kind, Some(CompletionItemKind::FIELD));
    assert_eq!(items[0].detail.as_deref(), Some("field value: Int"));
}

#[test]
fn package_bridge_completes_dependency_struct_member_fields_without_semantic_analysis() {
    let temp = TempDir::new("ql-lsp-package-struct-member-field-broken-completion");
    let app_root = temp.path().join("workspace").join("app");
    let source = r#"
package demo.app

use demo.dep.Config as Cfg

pub fn read(config: Cfg) -> Int {
    let built = Cfg { value: 1 }
    let broken: Int = "oops"
    return config.va + built.va
}
"#;
    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Config {
    value: Int,
}

impl Config {
    pub fn get(self) -> Int
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");

    let Some(CompletionResponse::Array(items)) = completion_for_dependency_member_fields(
        source,
        &package,
        offset_to_position(source, nth_offset(source, ".va", 1) + ".va".len()),
    ) else {
        panic!("dependency struct member field completion should exist without semantic analysis");
    };
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "value");
    assert_eq!(items[0].kind, Some(CompletionItemKind::FIELD));
    assert_eq!(items[0].detail.as_deref(), Some("field value: Int"));
}

#[test]
fn package_bridge_completes_dependency_value_root_members_in_parse_error_source() {
    let temp = TempDir::new("ql-lsp-package-value-root-member-completion-parse-error");
    let app_root = temp.path().join("workspace").join("app");
    let source = r#"
package demo.app

use demo.dep.Config as Cfg

pub fn read(config: Cfg) -> Int {
    let current = config
    return current.va + current.ge(
"#;
    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Config {
    value: Int,
}

impl Config {
    pub fn get(self) -> Int
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");
    assert!(analyze_source(source).is_err());

    let Some(CompletionResponse::Array(field_items)) = completion_for_dependency_member_fields(
        source,
        &package,
        offset_to_position(source, nth_offset(source, ".va", 1) + ".va".len()),
    ) else {
        panic!("dependency value-root field completion should exist in parse-error source");
    };
    assert_eq!(field_items.len(), 1);
    assert_eq!(field_items[0].label, "value");
    assert_eq!(field_items[0].kind, Some(CompletionItemKind::FIELD));
    assert_eq!(field_items[0].detail.as_deref(), Some("field value: Int"));

    let Some(CompletionResponse::Array(method_items)) = completion_for_dependency_methods(
        source,
        &package,
        offset_to_position(source, nth_offset(source, ".ge", 1) + ".ge".len()),
    ) else {
        panic!("dependency value-root method completion should exist in parse-error source");
    };
    assert_eq!(method_items.len(), 1);
    assert_eq!(method_items[0].label, "get");
    assert_eq!(method_items[0].kind, Some(CompletionItemKind::METHOD));
    assert_eq!(
        method_items[0].detail.as_deref(),
        Some("fn get(self) -> Int")
    );
}

#[test]
fn package_bridge_completes_dependency_direct_question_receiver_members_in_parse_error_source() {
    let temp = TempDir::new("ql-lsp-package-direct-question-member-completion-parse-error");
    let app_root = temp.path().join("workspace").join("app");
    let source = r#"
package demo.app

use demo.dep.Config as Cfg

pub fn read(config: Cfg) -> Int {
    return config.child?.va + config.child()?.ge(
}
"#;
    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Child {
    value: Int,
}

pub struct ErrInfo {
    code: Int,
}

pub struct Config {
    child: Option[Child],
}

impl Config {
    pub fn child(self) -> Result[Child, ErrInfo]
}

impl Child {
    pub fn get(self) -> Int
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");
    assert!(analyze_source(source).is_err());

    let Some(CompletionResponse::Array(field_items)) = completion_for_dependency_member_fields(
        source,
        &package,
        offset_to_position(source, nth_offset(source, ".va", 1) + ".va".len()),
    ) else {
        panic!("dependency direct-question field completion should exist in parse-error source");
    };
    assert_eq!(field_items.len(), 1);
    assert_eq!(field_items[0].label, "value");
    assert_eq!(field_items[0].kind, Some(CompletionItemKind::FIELD));
    assert_eq!(field_items[0].detail.as_deref(), Some("field value: Int"));

    let Some(CompletionResponse::Array(method_items)) = completion_for_dependency_methods(
        source,
        &package,
        offset_to_position(source, nth_offset(source, ".ge", 1) + ".ge".len()),
    ) else {
        panic!("dependency direct-question method completion should exist in parse-error source");
    };
    assert_eq!(method_items.len(), 1);
    assert_eq!(method_items[0].label, "get");
    assert_eq!(method_items[0].kind, Some(CompletionItemKind::METHOD));
    assert_eq!(
        method_items[0].detail.as_deref(),
        Some("fn get(self) -> Int")
    );
}

#[test]
fn package_bridge_completes_dependency_import_call_result_members_in_parse_error_source() {
    let temp = TempDir::new("ql-lsp-package-import-call-member-completion-parse-error");
    let app_root = temp.path().join("workspace").join("app");
    let source = r#"
package demo.app

use demo.dep.load
use demo.dep.maybe_load

pub fn read() -> Int {
    let first = load().va
    let second = load().ge(
    let third = maybe_load()?.va
    let fourth = maybe_load()?.ge(
}
"#;
    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Child {
    value: Int,
}

impl Child {
    pub fn get(self) -> Int
}

pub fn load() -> Child
pub fn maybe_load() -> Option[Child]
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");
    assert!(analyze_source(source).is_err());

    let Some(CompletionResponse::Array(load_field_items)) = completion_for_dependency_member_fields(
        source,
        &package,
        offset_to_position(source, nth_offset(source, ".va", 1) + ".va".len()),
    ) else {
        panic!("dependency import call-result field completion should exist in parse-error source");
    };
    assert_eq!(load_field_items.len(), 1);
    assert_eq!(load_field_items[0].label, "value");
    assert_eq!(load_field_items[0].kind, Some(CompletionItemKind::FIELD));
    assert_eq!(
        load_field_items[0].detail.as_deref(),
        Some("field value: Int")
    );

    let Some(CompletionResponse::Array(load_method_items)) = completion_for_dependency_methods(
        source,
        &package,
        offset_to_position(source, nth_offset(source, ".ge", 1) + ".ge".len()),
    ) else {
        panic!(
            "dependency import call-result method completion should exist in parse-error source"
        );
    };
    assert_eq!(load_method_items.len(), 1);
    assert_eq!(load_method_items[0].label, "get");
    assert_eq!(load_method_items[0].kind, Some(CompletionItemKind::METHOD));
    assert_eq!(
        load_method_items[0].detail.as_deref(),
        Some("fn get(self) -> Int")
    );

    let Some(CompletionResponse::Array(maybe_field_items)) =
        completion_for_dependency_member_fields(
            source,
            &package,
            offset_to_position(source, nth_offset(source, ".va", 2) + ".va".len()),
        )
    else {
        panic!(
            "dependency import question-call field completion should exist in parse-error source"
        );
    };
    assert_eq!(maybe_field_items.len(), 1);
    assert_eq!(maybe_field_items[0].label, "value");
    assert_eq!(maybe_field_items[0].kind, Some(CompletionItemKind::FIELD));
    assert_eq!(
        maybe_field_items[0].detail.as_deref(),
        Some("field value: Int")
    );

    let Some(CompletionResponse::Array(maybe_method_items)) = completion_for_dependency_methods(
        source,
        &package,
        offset_to_position(source, nth_offset(source, ".ge", 2) + ".ge".len()),
    ) else {
        panic!(
            "dependency import question-call method completion should exist in parse-error source"
        );
    };
    assert_eq!(maybe_method_items.len(), 1);
    assert_eq!(maybe_method_items[0].label, "get");
    assert_eq!(maybe_method_items[0].kind, Some(CompletionItemKind::METHOD));
    assert_eq!(
        maybe_method_items[0].detail.as_deref(),
        Some("fn get(self) -> Int")
    );
}

#[test]
fn package_bridge_surfaces_dependency_import_call_member_queries_in_parse_error_source() {
    let temp = TempDir::new("ql-lsp-package-import-call-member-queries-parse-error");
    let app_root = temp.path().join("workspace").join("app");
    let app_path = temp
        .path()
        .join("workspace")
        .join("app")
        .join("src")
        .join("lib.ql");
    let source = r#"
package demo.app

use demo.dep.load
use demo.dep.maybe_load

pub fn read() -> Int {
    let first = load().value
    let second = load().get()
    let third = maybe_load()?.value
    let fourth = maybe_load()?.get()
    let fifth = load().value
    let sixth = maybe_load()?.get(
}
"#;

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    let dep_qi = temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Child {
    value: Int,
}

impl Child {
    pub fn get(self) -> Int
}

pub fn load() -> Child
pub fn maybe_load() -> Option[Child]
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");

    let direct_field_hover = hover_for_dependency_struct_fields(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "value", 1)),
    )
    .expect("dependency import call-result field hover should exist in parse-error source");
    let HoverContents::Markup(direct_field_markup) = direct_field_hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(direct_field_markup.value.contains("**field** `value`"));
    assert!(direct_field_markup.value.contains("field value: Int"));

    let question_field_hover = hover_for_dependency_struct_fields(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "value", 3)),
    )
    .expect("dependency import question-call field hover should exist in parse-error source");
    let HoverContents::Markup(question_field_markup) = question_field_hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(question_field_markup.value.contains("**field** `value`"));
    assert!(question_field_markup.value.contains("field value: Int"));

    let direct_field_definition = definition_for_dependency_struct_fields(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "value", 1)),
    )
    .expect("dependency import call-result field definition should exist in parse-error source");
    let GotoDefinitionResponse::Scalar(Location {
        uri: direct_field_definition_uri,
        range: direct_field_definition_range,
    }) = direct_field_definition
    else {
        panic!("definition should be one location")
    };
    assert_eq!(
        direct_field_definition_uri
            .to_file_path()
            .expect("definition URI should convert to a file path")
            .canonicalize()
            .expect("definition path should canonicalize"),
        dep_qi
            .canonicalize()
            .expect("dependency artifact path should canonicalize"),
    );

    let direct_method_hover = hover_for_dependency_methods(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "get", 1)),
    )
    .expect("dependency import call-result method hover should exist in parse-error source");
    let HoverContents::Markup(direct_method_markup) = direct_method_hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(direct_method_markup.value.contains("**method** `get`"));
    assert!(direct_method_markup.value.contains("fn get(self) -> Int"));

    let question_method_hover = hover_for_dependency_methods(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "get", 2)),
    )
    .expect("dependency import question-call method hover should exist in parse-error source");
    let HoverContents::Markup(question_method_markup) = question_method_hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(question_method_markup.value.contains("**method** `get`"));
    assert!(question_method_markup.value.contains("fn get(self) -> Int"));

    let question_method_definition = definition_for_dependency_methods(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "get", 2)),
    )
    .expect("dependency import question-call method definition should exist in parse-error source");
    let GotoDefinitionResponse::Scalar(Location {
        uri: question_method_definition_uri,
        range: question_method_definition_range,
    }) = question_method_definition
    else {
        panic!("definition should be one location")
    };
    assert_eq!(
        question_method_definition_uri
            .to_file_path()
            .expect("definition URI should convert to a file path")
            .canonicalize()
            .expect("definition path should canonicalize"),
        dep_qi
            .canonicalize()
            .expect("dependency artifact path should canonicalize"),
    );

    let declaration = declaration_for_dependency_methods(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "get", 1)),
    )
    .expect("dependency import call-result method declaration should exist in parse-error source");
    let GotoDeclarationResponse::Scalar(declaration_location) = declaration else {
        panic!("declaration should be one location")
    };
    assert_eq!(declaration_location.uri, question_method_definition_uri);
    assert_eq!(declaration_location.range, question_method_definition_range);

    let artifact = fs::read_to_string(&dep_qi)
        .expect("dependency interface artifact should exist")
        .replace("\r\n", "\n");
    let field_start = artifact
        .find("value")
        .expect("field name should exist in dependency artifact");
    assert_eq!(
        direct_field_definition_range,
        span_to_range(
            &artifact,
            Span::new(field_start, field_start + "value".len())
        )
    );
    let method_start = artifact
        .find("pub fn get(self) -> Int")
        .map(|offset| offset + "pub fn ".len())
        .expect("method signature should exist in dependency artifact");
    assert_eq!(
        question_method_definition_range,
        span_to_range(
            &artifact,
            Span::new(method_start, method_start + "get".len())
        )
    );

    let field_references = references_for_dependency_struct_fields(
        &uri,
        source,
        &package,
        offset_to_position(source, nth_offset(source, "value", 1)),
        false,
    )
    .expect("dependency import call-result field references should exist in parse-error source");
    assert_eq!(
        field_references,
        vec![
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "value", 1),
                        nth_offset(source, "value", 1) + "value".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "value", 2),
                        nth_offset(source, "value", 2) + "value".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "value", 3),
                        nth_offset(source, "value", 3) + "value".len(),
                    ),
                ),
            ),
        ]
    );

    let method_references = references_for_dependency_methods(
        &uri,
        source,
        &package,
        offset_to_position(source, nth_offset(source, "get", 2)),
        false,
    )
    .expect("dependency import question-call method references should exist in parse-error source");
    assert_eq!(
        method_references,
        vec![
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "get", 1),
                        nth_offset(source, "get", 1) + "get".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "get", 2),
                        nth_offset(source, "get", 2) + "get".len(),
                    ),
                ),
            ),
            Location::new(
                uri,
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "get", 3),
                        nth_offset(source, "get", 3) + "get".len(),
                    ),
                ),
            ),
        ]
    );
}

#[test]
fn package_bridge_surfaces_dependency_direct_question_member_queries_in_parse_error_source() {
    let temp = TempDir::new("ql-lsp-package-direct-question-member-queries-parse-error");
    let app_root = temp.path().join("workspace").join("app");
    let app_path = temp
        .path()
        .join("workspace")
        .join("app")
        .join("src")
        .join("lib.ql");
    let source = r#"
package demo.app

use demo.dep.Config as Cfg

pub fn read(config: Cfg) -> Int {
    let first = config.child?.value
    let second = config.child()?.get()
    let third = config.child?.value
    let fourth = config.child()?.get(
}
"#;

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    let dep_qi = temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Child {
    value: Int,
}

pub struct ErrInfo {
    code: Int,
}

pub struct Config {
    child: Option[Child],
}

impl Config {
    pub fn child(self) -> Result[Child, ErrInfo]
}

impl Child {
    pub fn get(self) -> Int
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");

    let field_hover = hover_for_dependency_struct_fields(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "value", 1)),
    )
    .expect("dependency direct-question field hover should exist in parse-error source");
    let HoverContents::Markup(field_markup) = field_hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(field_markup.value.contains("**field** `value`"));
    assert!(field_markup.value.contains("field value: Int"));

    let field_definition = definition_for_dependency_struct_fields(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "value", 2)),
    )
    .expect("dependency direct-question field definition should exist in parse-error source");
    let GotoDefinitionResponse::Scalar(Location {
        uri: field_definition_uri,
        range: field_definition_range,
    }) = field_definition
    else {
        panic!("definition should be one location")
    };
    assert_eq!(
        field_definition_uri
            .to_file_path()
            .expect("definition URI should convert to a file path")
            .canonicalize()
            .expect("definition path should canonicalize"),
        dep_qi
            .canonicalize()
            .expect("dependency artifact path should canonicalize"),
    );

    let method_hover = hover_for_dependency_methods(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "get", 1)),
    )
    .expect("dependency direct-question method hover should exist in parse-error source");
    let HoverContents::Markup(method_markup) = method_hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(method_markup.value.contains("**method** `get`"));
    assert!(method_markup.value.contains("fn get(self) -> Int"));

    let method_definition = definition_for_dependency_methods(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "get", 2)),
    )
    .expect("dependency direct-question method definition should exist in parse-error source");
    let GotoDefinitionResponse::Scalar(Location {
        uri: method_definition_uri,
        range: method_definition_range,
    }) = method_definition
    else {
        panic!("definition should be one location")
    };
    assert_eq!(
        method_definition_uri
            .to_file_path()
            .expect("definition URI should convert to a file path")
            .canonicalize()
            .expect("definition path should canonicalize"),
        dep_qi
            .canonicalize()
            .expect("dependency artifact path should canonicalize"),
    );

    let declaration = declaration_for_dependency_methods(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "get", 1)),
    )
    .expect("dependency direct-question method declaration should exist in parse-error source");
    let GotoDeclarationResponse::Scalar(declaration_location) = declaration else {
        panic!("declaration should be one location")
    };
    assert_eq!(declaration_location.uri, method_definition_uri);
    assert_eq!(declaration_location.range, method_definition_range);

    let artifact = fs::read_to_string(&dep_qi)
        .expect("dependency interface artifact should exist")
        .replace("\r\n", "\n");
    let field_start = artifact
        .find("value")
        .expect("field name should exist in dependency artifact");
    assert_eq!(
        field_definition_range,
        span_to_range(
            &artifact,
            Span::new(field_start, field_start + "value".len())
        )
    );
    let method_start = artifact
        .find("pub fn get(self) -> Int")
        .map(|offset| offset + "pub fn ".len())
        .expect("method signature should exist in dependency artifact");
    assert_eq!(
        method_definition_range,
        span_to_range(
            &artifact,
            Span::new(method_start, method_start + "get".len())
        )
    );

    let field_references = references_for_dependency_struct_fields(
        &uri,
        source,
        &package,
        offset_to_position(source, nth_offset(source, "value", 1)),
        false,
    )
    .expect("dependency direct-question field references should exist in parse-error source");
    assert_eq!(
        field_references,
        vec![
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "value", 1),
                        nth_offset(source, "value", 1) + "value".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "value", 2),
                        nth_offset(source, "value", 2) + "value".len(),
                    ),
                ),
            ),
        ]
    );

    let method_references = references_for_dependency_methods(
        &uri,
        source,
        &package,
        offset_to_position(source, nth_offset(source, "get", 1)),
        false,
    )
    .expect("dependency direct-question method references should exist in parse-error source");
    assert_eq!(
        method_references,
        vec![
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "get", 1),
                        nth_offset(source, "get", 1) + "get".len(),
                    ),
                ),
            ),
            Location::new(
                uri,
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "get", 2),
                        nth_offset(source, "get", 2) + "get".len(),
                    ),
                ),
            ),
        ]
    );
}

#[test]
fn package_bridge_follows_dependency_direct_question_member_types_in_parse_error_source() {
    let temp = TempDir::new("ql-lsp-package-direct-question-member-type-definition-parse-error");
    let app_root = temp.path().join("workspace").join("app");
    let source = r#"
package demo.app

use demo.dep.Config as Cfg

pub fn read(config: Cfg) -> Int {
    let field = config.child?.leaf
    let method = config.child()?.leaf()
    let broken = config.child()?.leaf(
}
"#;

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    let dep_qi = temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Leaf {
    value: Int,
}

pub struct Child {
    leaf: Leaf,
}

pub struct ErrInfo {
    code: Int,
}

pub struct Config {
    child: Option[Child],
}

impl Config {
    pub fn child(self) -> Result[Child, ErrInfo]
}

impl Child {
    pub fn leaf(self) -> Leaf
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");

    let field_type = type_definition_for_dependency_struct_field_types(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "leaf", 1)),
    )
    .expect("dependency direct-question field type definition should exist in parse-error source");
    let GotoTypeDefinitionResponse::Scalar(field_location) = field_type else {
        panic!("type definition should be one location")
    };
    assert_eq!(
        field_location
            .uri
            .to_file_path()
            .expect("definition URI should convert to a file path")
            .canonicalize()
            .expect("definition path should canonicalize"),
        dep_qi
            .canonicalize()
            .expect("dependency artifact path should canonicalize"),
    );
    let artifact = fs::read_to_string(&dep_qi)
        .expect("dependency interface artifact should exist")
        .replace("\r\n", "\n");
    let anchor = "pub struct Leaf {\n    value: Int,\n}";
    let anchor_start = artifact
        .find(anchor)
        .expect("leaf struct should exist in dependency artifact");
    assert_eq!(
        field_location.range,
        span_to_range(
            &artifact,
            Span::new(anchor_start, anchor_start + anchor.len())
        )
    );

    let method_type = type_definition_for_dependency_method_types(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "leaf", 2)),
    )
    .expect("dependency direct-question method type definition should exist in parse-error source");
    let GotoTypeDefinitionResponse::Scalar(method_location) = method_type else {
        panic!("type definition should be one location")
    };
    assert_eq!(method_location, field_location);
}

#[test]
fn package_bridge_surfaces_dependency_direct_indexed_receiver_field_queries_in_parse_error_source()
{
    let temp = TempDir::new("ql-lsp-package-direct-indexed-receiver-field-query-parse-error");
    let app_root = temp.path().join("workspace").join("app");
    let app_path = temp
        .path()
        .join("workspace")
        .join("app")
        .join("src")
        .join("lib.ql");
    let source = r#"
package demo.app

use demo.dep.Config as Cfg

pub fn read(config: Cfg) -> Int {
    let first = config.children[0].value
    let second = config.children[1].value
    let broken = config.children[0].value
"#;

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    let dep_qi = temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Child {
    value: Int,
}

pub struct Config {
    children: [Child; 2],
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");

    let hover = hover_for_dependency_struct_fields(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "value", 1)),
    )
    .expect("dependency direct-indexed field hover should exist in parse-error source");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(markup.value.contains("**field** `value`"));
    assert!(markup.value.contains("field value: Int"));

    let definition = definition_for_dependency_struct_fields(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "value", 2)),
    )
    .expect("dependency direct-indexed field definition should exist in parse-error source");
    let GotoDefinitionResponse::Scalar(location) = definition else {
        panic!("definition should be one location")
    };
    assert_eq!(
        location
            .uri
            .to_file_path()
            .expect("definition URI should convert to a file path")
            .canonicalize()
            .expect("definition path should canonicalize"),
        dep_qi
            .canonicalize()
            .expect("dependency artifact path should canonicalize"),
    );
    let artifact = fs::read_to_string(&dep_qi)
        .expect("dependency interface artifact should exist")
        .replace("\r\n", "\n");
    let field_start = artifact
        .find("value")
        .expect("field name should exist in dependency artifact");
    assert_eq!(
        location.range,
        span_to_range(
            &artifact,
            Span::new(field_start, field_start + "value".len())
        )
    );

    let declaration = declaration_for_dependency_struct_fields(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "value", 3)),
    )
    .expect("dependency direct-indexed field declaration should exist in parse-error source");
    let GotoDeclarationResponse::Scalar(declaration_location) = declaration else {
        panic!("declaration should be one location")
    };
    assert_eq!(declaration_location, location);

    let references = references_for_dependency_struct_fields(
        &uri,
        source,
        &package,
        offset_to_position(source, nth_offset(source, "value", 1)),
        false,
    )
    .expect("dependency direct-indexed field references should exist in parse-error source");
    assert_eq!(
        references,
        vec![
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "value", 1),
                        nth_offset(source, "value", 1) + "value".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "value", 2),
                        nth_offset(source, "value", 2) + "value".len(),
                    ),
                ),
            ),
            Location::new(
                uri,
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "value", 3),
                        nth_offset(source, "value", 3) + "value".len(),
                    ),
                ),
            ),
        ]
    );
}

#[test]
fn package_bridge_surfaces_dependency_root_indexed_receiver_field_queries_in_parse_error_source() {
    let temp = TempDir::new("ql-lsp-package-root-indexed-receiver-field-query-parse-error");
    let app_root = temp.path().join("workspace").join("app");
    let app_path = temp
        .path()
        .join("workspace")
        .join("app")
        .join("src")
        .join("lib.ql");
    let source = r#"
package demo.app

use demo.dep.load_children
use demo.dep.maybe_children

pub fn read() -> Int {
    let first = load_children()[0].value
    let second = maybe_children()?[1].value
    let third = load_children()[1].value
    let fourth = maybe_children()?[0].value
    let broken = maybe_children()?[0].value
"#;

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    let dep_qi = temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Child {
    value: Int,
}

pub fn load_children() -> [Child; 2]
pub fn maybe_children() -> Option[[Child; 2]]
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");

    let direct_hover = hover_for_dependency_struct_fields(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "value", 1)),
    )
    .expect("dependency root-indexed call field hover should exist in parse-error source");
    let HoverContents::Markup(direct_markup) = direct_hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(direct_markup.value.contains("**field** `value`"));
    assert!(direct_markup.value.contains("field value: Int"));

    let question_hover = hover_for_dependency_struct_fields(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "value", 2)),
    )
    .expect("dependency root-indexed question-call field hover should exist in parse-error source");
    let HoverContents::Markup(question_markup) = question_hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(question_markup.value.contains("**field** `value`"));
    assert!(question_markup.value.contains("field value: Int"));

    let definition = definition_for_dependency_struct_fields(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "value", 3)),
    )
    .expect("dependency root-indexed call field definition should exist in parse-error source");
    let GotoDefinitionResponse::Scalar(location) = definition else {
        panic!("definition should be one location")
    };
    assert_eq!(
        location
            .uri
            .to_file_path()
            .expect("definition URI should convert to a file path")
            .canonicalize()
            .expect("definition path should canonicalize"),
        dep_qi
            .canonicalize()
            .expect("dependency artifact path should canonicalize"),
    );
    let artifact = fs::read_to_string(&dep_qi)
        .expect("dependency interface artifact should exist")
        .replace("\r\n", "\n");
    let field_start = artifact
        .find("value")
        .expect("field name should exist in dependency artifact");
    assert_eq!(
        location.range,
        span_to_range(
            &artifact,
            Span::new(field_start, field_start + "value".len())
        )
    );

    let declaration = declaration_for_dependency_struct_fields(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "value", 4)),
    )
    .expect("dependency root-indexed question-call field declaration should exist in parse-error source");
    let GotoDeclarationResponse::Scalar(declaration_location) = declaration else {
        panic!("declaration should be one location")
    };
    assert_eq!(declaration_location, location);

    let references = references_for_dependency_struct_fields(
        &uri,
        source,
        &package,
        offset_to_position(source, nth_offset(source, "value", 5)),
        false,
    )
    .expect("dependency root-indexed field references should exist in parse-error source");
    assert_eq!(
        references,
        vec![
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "value", 1),
                        nth_offset(source, "value", 1) + "value".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "value", 2),
                        nth_offset(source, "value", 2) + "value".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "value", 3),
                        nth_offset(source, "value", 3) + "value".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "value", 4),
                        nth_offset(source, "value", 4) + "value".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "value", 5),
                        nth_offset(source, "value", 5) + "value".len(),
                    ),
                ),
            ),
        ]
    );
}

#[test]
fn package_bridge_surfaces_dependency_root_indexed_value_queries_in_parse_error_source() {
    let temp = TempDir::new("ql-lsp-package-root-indexed-value-query-parse-error");
    let app_root = temp.path().join("workspace").join("app");
    let app_path = temp
        .path()
        .join("workspace")
        .join("app")
        .join("src")
        .join("lib.ql");
    let source = r#"
package demo.app

use demo.dep.load_children
use demo.dep.maybe_children

pub fn read() -> Int {
    let first = load_children()[0]
    let second = maybe_children()?[1]
    return first.value + second.value + first.value
"#;

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    let dep_qi = temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Child {
    value: Int,
}

pub fn load_children() -> [Child; 2]
pub fn maybe_children() -> Option[[Child; 2]]
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");

    let direct_hover = hover_for_dependency_values(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "first", 2)),
    )
    .expect("dependency root-indexed value hover should exist in parse-error source");
    let HoverContents::Markup(direct_markup) = direct_hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(direct_markup.value.contains("**struct** `Child`"));
    assert!(direct_markup.value.contains("struct Child"));

    let question_definition = definition_for_dependency_values(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "second", 2)),
    )
    .expect("dependency root-indexed question value definition should exist in parse-error source");
    let GotoDefinitionResponse::Scalar(question_location) = question_definition else {
        panic!("definition should be one location")
    };
    assert_eq!(
        question_location
            .uri
            .to_file_path()
            .expect("definition URI should convert to a file path")
            .canonicalize()
            .expect("definition path should canonicalize"),
        dep_qi
            .canonicalize()
            .expect("dependency artifact path should canonicalize"),
    );
    let artifact = fs::read_to_string(&dep_qi)
        .expect("dependency interface artifact should exist")
        .replace("\r\n", "\n");
    let struct_start = artifact
        .find("pub struct Child {\n    value: Int,\n}")
        .expect("struct declaration should exist in dependency artifact");
    assert_eq!(
        question_location.range,
        span_to_range(
            &artifact,
            Span::new(
                struct_start,
                struct_start + "pub struct Child {\n    value: Int,\n}".len(),
            )
        )
    );

    let question_type_definition = type_definition_for_dependency_values(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "second", 2)),
    )
    .expect(
        "dependency root-indexed question value type definition should exist in parse-error source",
    );
    let GotoTypeDefinitionResponse::Scalar(type_location) = question_type_definition else {
        panic!("type definition should be one location")
    };
    assert_eq!(type_location, question_location);

    let references = references_for_dependency_values(
        &uri,
        source,
        &package,
        offset_to_position(source, nth_offset(source, "first", 1)),
        false,
    )
    .expect("dependency root-indexed value references should exist in parse-error source");
    assert_eq!(
        references,
        vec![
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "first", 2),
                        nth_offset(source, "first", 2) + "first".len(),
                    ),
                ),
            ),
            Location::new(
                uri,
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "first", 3),
                        nth_offset(source, "first", 3) + "first".len(),
                    ),
                ),
            ),
        ]
    );
}

#[test]
fn package_bridge_surfaces_dependency_structured_root_indexed_value_queries_in_parse_error_source()
{
    let temp = TempDir::new("ql-lsp-package-structured-root-indexed-value-query-parse-error");
    let app_root = temp.path().join("workspace").join("app");
    let app_path = temp
        .path()
        .join("workspace")
        .join("app")
        .join("src")
        .join("lib.ql");
    let source = r#"
package demo.app

use demo.dep.maybe_children

pub fn read(flag: Bool) -> Int {
    let first = (if flag { maybe_children()? } else { maybe_children()? })[0]
    let second = (match flag { true => maybe_children()?, false => maybe_children()? })[1]
    return first.value + second.value + first.value
"#;

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    let dep_qi = temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Child {
    value: Int,
}

pub fn maybe_children() -> Option[[Child; 2]]
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");

    let direct_hover = hover_for_dependency_values(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "first", 2)),
    )
    .expect("dependency structured root-indexed value hover should exist in parse-error source");
    let HoverContents::Markup(direct_markup) = direct_hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(direct_markup.value.contains("**struct** `Child`"));
    assert!(direct_markup.value.contains("struct Child"));

    let match_definition = definition_for_dependency_values(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "second", 2)),
    )
    .expect(
        "dependency structured match root-indexed value definition should exist in parse-error source",
    );
    let GotoDefinitionResponse::Scalar(match_location) = match_definition else {
        panic!("definition should be one location")
    };
    assert_eq!(
        match_location
            .uri
            .to_file_path()
            .expect("definition URI should convert to a file path")
            .canonicalize()
            .expect("definition path should canonicalize"),
        dep_qi
            .canonicalize()
            .expect("dependency artifact path should canonicalize"),
    );
    let artifact = fs::read_to_string(&dep_qi)
        .expect("dependency interface artifact should exist")
        .replace("\r\n", "\n");
    let struct_start = artifact
        .find("pub struct Child {\n    value: Int,\n}")
        .expect("struct declaration should exist in dependency artifact");
    assert_eq!(
        match_location.range,
        span_to_range(
            &artifact,
            Span::new(
                struct_start,
                struct_start + "pub struct Child {\n    value: Int,\n}".len(),
            )
        )
    );

    let match_type_definition = type_definition_for_dependency_values(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "second", 2)),
    )
    .expect(
        "dependency structured match root-indexed value type definition should exist in parse-error source",
    );
    let GotoTypeDefinitionResponse::Scalar(type_location) = match_type_definition else {
        panic!("type definition should be one location")
    };
    assert_eq!(type_location, match_location);

    let references = references_for_dependency_values(
        &uri,
        source,
        &package,
        offset_to_position(source, nth_offset(source, "first", 1)),
        false,
    )
    .expect(
        "dependency structured root-indexed value references should exist in parse-error source",
    );
    assert_eq!(
        references,
        vec![
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "first", 2),
                        nth_offset(source, "first", 2) + "first".len(),
                    ),
                ),
            ),
            Location::new(
                uri,
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "first", 3),
                        nth_offset(source, "first", 3) + "first".len(),
                    ),
                ),
            ),
        ]
    );
}

#[test]
fn package_bridge_surfaces_dependency_structured_root_indexed_member_queries_in_parse_error_source()
{
    let temp = TempDir::new("ql-lsp-package-structured-root-indexed-member-query-parse-error");
    let app_root = temp.path().join("workspace").join("app");
    let app_path = temp
        .path()
        .join("workspace")
        .join("app")
        .join("src")
        .join("lib.ql");
    let source = r#"
package demo.app

use demo.dep.maybe_children

pub fn read(flag: Bool) -> Int {
    let first = (if flag { maybe_children()? } else { maybe_children()? })[0].leaf.value
    let second = (match flag { true => maybe_children()?, false => maybe_children()? })[1].leaf()
    let third = (if flag { maybe_children()? } else { maybe_children()? })[1].leaf.value
    let broken = (match flag { true => maybe_children()?, false => maybe_children()? })[0].leaf(
"#;

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    let dep_qi = temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Leaf {
    value: Int,
}

pub struct Child {
    leaf: Leaf,
}

impl Child {
    pub fn leaf(self) -> Leaf
}

pub fn maybe_children() -> Option[[Child; 2]]
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");

    let field_hover = hover_for_dependency_struct_fields(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "leaf", 1)),
    )
    .expect("dependency structured root-indexed field hover should exist in parse-error source");
    let HoverContents::Markup(field_markup) = field_hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(field_markup.value.contains("**field** `leaf`"));
    assert!(field_markup.value.contains("field leaf: Leaf"));

    let field_definition = definition_for_dependency_struct_fields(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "leaf", 3)),
    )
    .expect(
        "dependency structured root-indexed field definition should exist in parse-error source",
    );
    let GotoDefinitionResponse::Scalar(field_location) = field_definition else {
        panic!("definition should be one location")
    };
    assert_eq!(
        field_location
            .uri
            .to_file_path()
            .expect("definition URI should convert to a file path")
            .canonicalize()
            .expect("definition path should canonicalize"),
        dep_qi
            .canonicalize()
            .expect("dependency artifact path should canonicalize"),
    );

    let method_hover = hover_for_dependency_methods(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "leaf", 2)),
    )
    .expect("dependency structured root-indexed method hover should exist in parse-error source");
    let HoverContents::Markup(method_markup) = method_hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(method_markup.value.contains("**method** `leaf`"));
    assert!(method_markup.value.contains("fn leaf(self) -> Leaf"));

    let method_definition = definition_for_dependency_methods(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "leaf", 4)),
    )
    .expect(
        "dependency structured root-indexed method definition should exist in parse-error source",
    );
    let GotoDefinitionResponse::Scalar(method_location) = method_definition else {
        panic!("definition should be one location")
    };
    assert_eq!(
        method_location
            .uri
            .to_file_path()
            .expect("definition URI should convert to a file path")
            .canonicalize()
            .expect("definition path should canonicalize"),
        dep_qi
            .canonicalize()
            .expect("dependency artifact path should canonicalize"),
    );

    let artifact = fs::read_to_string(&dep_qi)
        .expect("dependency interface artifact should exist")
        .replace("\r\n", "\n");
    let field_start = artifact
        .find("leaf: Leaf")
        .expect("field declaration should exist in dependency artifact");
    let field_name_start = field_start;
    assert_eq!(
        field_location.range,
        span_to_range(
            &artifact,
            Span::new(field_name_start, field_name_start + "leaf".len())
        )
    );
    let method_start = artifact
        .find("pub fn leaf(self) -> Leaf")
        .map(|offset| offset + "pub fn ".len())
        .expect("method signature should exist in dependency artifact");
    assert_eq!(
        method_location.range,
        span_to_range(
            &artifact,
            Span::new(method_start, method_start + "leaf".len())
        )
    );

    let field_references = references_for_dependency_struct_fields(
        &uri,
        source,
        &package,
        offset_to_position(source, nth_offset(source, "leaf", 1)),
        false,
    )
    .expect(
        "dependency structured root-indexed field references should exist in parse-error source",
    );
    assert_eq!(
        field_references,
        vec![
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "leaf", 1),
                        nth_offset(source, "leaf", 1) + "leaf".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "leaf", 3),
                        nth_offset(source, "leaf", 3) + "leaf".len(),
                    ),
                ),
            ),
        ]
    );

    let method_references = references_for_dependency_methods(
        &uri,
        source,
        &package,
        offset_to_position(source, nth_offset(source, "leaf", 2)),
        false,
    )
    .expect(
        "dependency structured root-indexed method references should exist in parse-error source",
    );
    assert_eq!(
        method_references,
        vec![
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "leaf", 2),
                        nth_offset(source, "leaf", 2) + "leaf".len(),
                    ),
                ),
            ),
            Location::new(
                uri,
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "leaf", 4),
                        nth_offset(source, "leaf", 4) + "leaf".len(),
                    ),
                ),
            ),
        ]
    );
}

#[test]
fn package_bridge_completes_dependency_struct_member_fields_for_closure_parameters() {
    let temp = TempDir::new("ql-lsp-package-struct-member-field-closure-param");
    let app_root = temp.path().join("workspace").join("app");
    let source = r#"
package demo.app

use demo.dep.Config as Cfg

pub fn read(config: Cfg) -> Int {
    let project = (current: Cfg) => current.va
    return project(config)
}
"#;
    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Config {
    value: Int,
}

impl Config {
    pub fn get(self) -> Int
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");
    let analysis = analyze_source(source).expect("analysis should succeed for completion query");

    let Some(CompletionResponse::Array(items)) = completion_for_package_analysis(
        source,
        &analysis,
        &package,
        offset_to_position(source, nth_offset(source, ".va", 1) + ".va".len()),
    ) else {
        panic!("dependency closure param field completion should exist");
    };
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "value");
    assert_eq!(items[0].kind, Some(CompletionItemKind::FIELD));
    assert_eq!(items[0].detail.as_deref(), Some("field value: Int"));
}

#[test]
fn package_bridge_surfaces_dependency_value_root_queries_for_closure_parameters() {
    let temp = TempDir::new("ql-lsp-package-value-root-closure-param");
    let app_root = temp.path().join("workspace").join("app");
    let app_path = temp
        .path()
        .join("workspace")
        .join("app")
        .join("src")
        .join("lib.ql");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    let dep_qi = temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Config {
    value: Int,
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    let source = r#"
package demo.app

use demo.dep.Config as Cfg

pub fn read(config: Cfg) -> Int {
    let project = (current: Cfg) => current.value + current.value
    return project(config)
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("analysis should succeed");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");
    let current_usage = nth_offset(source, "current", 2);

    let hover = hover_for_package_analysis(
        source,
        &analysis,
        &package,
        offset_to_position(source, current_usage),
    )
    .expect("dependency value root hover should exist");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(markup.value.contains("**struct** `Config`"));
    assert!(markup.value.contains("struct Config"));

    let definition = definition_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        offset_to_position(source, current_usage),
    )
    .expect("dependency value root definition should exist");
    let GotoDefinitionResponse::Scalar(Location {
        uri: definition_uri,
        range: definition_range,
    }) = definition
    else {
        panic!("definition should be one location")
    };
    assert_eq!(
        definition_uri
            .to_file_path()
            .expect("definition URI should convert to a file path")
            .canonicalize()
            .expect("definition path should canonicalize"),
        dep_qi
            .canonicalize()
            .expect("dependency artifact path should canonicalize"),
    );

    let artifact = fs::read_to_string(&dep_qi)
        .expect("dependency interface artifact should exist")
        .replace("\r\n", "\n");
    let snippet = "pub struct Config {\n    value: Int,\n}";
    let start = artifact
        .find(snippet)
        .expect("struct declaration should exist in dependency artifact");
    assert_eq!(
        definition_range,
        span_to_range(&artifact, Span::new(start, start + snippet.len()))
    );

    let declaration = declaration_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        offset_to_position(source, current_usage),
    )
    .expect("dependency value root declaration should exist");
    let GotoDeclarationResponse::Scalar(declaration_location) = declaration else {
        panic!("declaration should be one location")
    };
    assert_eq!(declaration_location.uri, definition_uri);
    assert_eq!(declaration_location.range, definition_range);

    let without_declaration = references_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        offset_to_position(source, current_usage),
        false,
    )
    .expect("dependency value root references should exist");
    assert_eq!(
        without_declaration,
        vec![
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "current", 2),
                        nth_offset(source, "current", 2) + "current".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "current", 3),
                        nth_offset(source, "current", 3) + "current".len(),
                    ),
                ),
            ),
        ]
    );

    let with_declaration = references_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        offset_to_position(source, current_usage),
        true,
    )
    .expect("dependency value root references with declaration should exist");
    assert_eq!(with_declaration.len(), 4);
    assert_eq!(
        with_declaration[0],
        Location::new(
            definition_uri.clone(),
            span_to_range(&artifact, Span::new(start, start + snippet.len())),
        ),
    );
    assert_eq!(
        with_declaration[1..],
        [
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "current", 1),
                        nth_offset(source, "current", 1) + "current".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "current", 2),
                        nth_offset(source, "current", 2) + "current".len(),
                    ),
                ),
            ),
            Location::new(
                uri,
                span_to_range(
                    source,
                    Span::new(
                        nth_offset(source, "current", 3),
                        nth_offset(source, "current", 3) + "current".len(),
                    ),
                ),
            ),
        ]
    );
}

#[test]
fn package_bridge_completes_dependency_struct_member_methods_for_closure_parameters_without_semantic_analysis()
 {
    let temp = TempDir::new("ql-lsp-package-struct-member-method-closure-param-broken");
    let app_root = temp.path().join("workspace").join("app");
    let source = r#"
package demo.app

use demo.dep.Config as Cfg

pub fn read(config: Cfg) -> Int {
    let project = (current: Cfg) => current.ge()
    let broken: Int = "oops"
    return project(config)
}
"#;
    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Config {
    value: Int,
}

impl Config {
    pub fn get(self) -> Int
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");

    let Some(CompletionResponse::Array(items)) = completion_for_dependency_methods(
        source,
        &package,
        offset_to_position(source, nth_offset(source, ".ge", 1) + ".ge".len()),
    ) else {
        panic!("dependency closure param method completion should exist without semantic analysis");
    };
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "get");
    assert_eq!(items[0].kind, Some(CompletionItemKind::METHOD));
    assert_eq!(items[0].detail.as_deref(), Some("fn get(self) -> Int"));
}

#[test]
fn package_bridge_surfaces_dependency_import_hover_and_definition_without_semantic_analysis() {
    let temp = TempDir::new("ql-lsp-package-import-broken-query");
    let app_root = temp.path().join("workspace").join("app");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    let dep_qi = temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub fn exported(value: Int) -> Int

pub struct Buffer[T] {
    value: T,
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    let source = r#"
package demo.app

use demo.dep.exported as run
use demo.dep.Buffer as Buf

pub fn main(value: Buf[Int]) -> Int {
    let next = run(1)
    return "oops"
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");

    let hover = hover_for_dependency_imports(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "run", 2)),
    )
    .expect("dependency import hover should exist even without semantic analysis");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(markup.value.contains("**function** `exported`"));
    assert!(markup.value.contains("fn exported(value: Int) -> Int"));

    let definition = definition_for_dependency_imports(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "Buf", 2)),
    )
    .expect("dependency import definition should exist even without semantic analysis");
    let GotoDefinitionResponse::Scalar(Location {
        uri: definition_uri,
        range,
    }) = definition
    else {
        panic!("definition should be one location")
    };
    assert_eq!(
        definition_uri
            .to_file_path()
            .expect("definition URI should convert to a file path")
            .canonicalize()
            .expect("definition path should canonicalize"),
        dep_qi
            .canonicalize()
            .expect("dependency artifact path should canonicalize"),
    );
    let artifact = fs::read_to_string(&dep_qi)
        .expect("dependency interface artifact should exist")
        .replace("\r\n", "\n");
    let snippet = "pub struct Buffer[T] {\n    value: T,\n}";
    let start = artifact
        .find(snippet)
        .expect("struct signature should exist in dependency artifact");
    assert_eq!(
        range,
        span_to_range(&artifact, Span::new(start, start + snippet.len()))
    );
}

#[test]
fn package_bridge_surfaces_dependency_import_hover_definition_and_type_definition_after_parse_errors()
 {
    let temp = TempDir::new("ql-lsp-package-import-parse-error-queries");
    let app_root = temp.path().join("workspace").join("app");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    let dep_qi = temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub fn exported(value: Int) -> Int

pub struct Buffer[T] {
    value: T,
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    let source = r#"
package demo.app

use demo.dep.exported as run
use demo.dep.Buffer as Buf

pub fn main(value: Buf[Int]) -> Int {
    let next = run(1)
    return Buf { value: next
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_source(source).is_err());
    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");

    let hover = hover_for_dependency_imports(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "run", 2)),
    )
    .expect("dependency import hover should exist after parse errors");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(markup.value.contains("**function** `exported`"));
    assert!(markup.value.contains("fn exported(value: Int) -> Int"));

    let definition = definition_for_dependency_imports(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "Buf", 2)),
    )
    .expect("dependency import definition should exist after parse errors");
    let GotoDefinitionResponse::Scalar(Location {
        uri: definition_uri,
        range: definition_range,
    }) = definition
    else {
        panic!("definition should be one location")
    };
    assert_eq!(
        definition_uri
            .to_file_path()
            .expect("definition URI should convert to a file path")
            .canonicalize()
            .expect("definition path should canonicalize"),
        dep_qi
            .canonicalize()
            .expect("dependency artifact path should canonicalize"),
    );

    let type_definition = type_definition_for_dependency_imports(
        source,
        &package,
        offset_to_position(source, nth_offset(source, "Buf", 2)),
    )
    .expect("dependency import type definition should exist after parse errors");
    let GotoTypeDefinitionResponse::Scalar(Location {
        uri: type_definition_uri,
        range: type_definition_range,
    }) = type_definition
    else {
        panic!("type definition should be one location")
    };
    assert_eq!(type_definition_uri, definition_uri);

    let artifact = fs::read_to_string(&dep_qi)
        .expect("dependency interface artifact should exist")
        .replace("\r\n", "\n");
    let snippet = "pub struct Buffer[T] {\n    value: T,\n}";
    let start = artifact
        .find(snippet)
        .expect("struct signature should exist in dependency artifact");
    let expected_range = span_to_range(&artifact, Span::new(start, start + snippet.len()));
    assert_eq!(definition_range, expected_range);
    assert_eq!(type_definition_range, expected_range);
}

#[test]
fn package_bridge_surfaces_dependency_import_references_without_semantic_analysis() {
    let temp = TempDir::new("ql-lsp-package-import-broken-references");
    let app_root = temp.path().join("workspace").join("app");
    let app_path = temp
        .path()
        .join("workspace")
        .join("app")
        .join("src")
        .join("lib.ql");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    let dep_qi = temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub fn exported(value: Int) -> Int
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    let source = r#"
package demo.app

use demo.dep.exported as run

pub fn main() -> Int {
    let next = run(1)
    return "oops"
}

pub fn later() -> Int {
    return run(2)
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");

    let with_declaration = references_for_dependency_imports(
        &uri,
        source,
        &package,
        offset_to_position(source, nth_offset(source, "run", 2)),
        true,
    )
    .expect("dependency import references should exist even without semantic analysis");
    assert_eq!(with_declaration.len(), 4);
    assert_eq!(
        with_declaration[0]
            .uri
            .to_file_path()
            .expect("definition URI should convert to a file path")
            .canonicalize()
            .expect("definition path should canonicalize"),
        dep_qi
            .canonicalize()
            .expect("dependency artifact path should canonicalize"),
    );
    assert!(
        with_declaration[1..]
            .iter()
            .all(|location| location.uri == uri)
    );

    let without_declaration = references_for_dependency_imports(
        &uri,
        source,
        &package,
        offset_to_position(source, nth_offset(source, "run", 3)),
        false,
    )
    .expect("dependency import references should exist without declaration");
    assert_eq!(without_declaration.len(), 2);
    assert!(
        without_declaration
            .iter()
            .all(|location| location.uri == uri)
    );
}

#[test]
fn package_bridge_surfaces_dependency_import_references_after_parse_errors() {
    let temp = TempDir::new("ql-lsp-package-import-parse-error-references");
    let app_root = temp.path().join("workspace").join("app");
    let app_path = temp
        .path()
        .join("workspace")
        .join("app")
        .join("src")
        .join("lib.ql");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    let dep_qi = temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub fn exported(value: Int) -> Int
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    let source = r#"
package demo.app

use demo.dep.exported as run

pub fn main() -> Int {
    let next = run(1)
    if true {
        return run(2)
    return 0
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_source(source).is_err());
    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");

    let references = references_for_dependency_imports(
        &uri,
        source,
        &package,
        offset_to_position(source, nth_offset(source, "run", 2)),
        true,
    )
    .expect("dependency import references should exist after parse errors");
    assert_eq!(references.len(), 4);
    assert_eq!(
        references[0]
            .uri
            .to_file_path()
            .expect("definition URI should convert to a file path")
            .canonicalize()
            .expect("definition path should canonicalize"),
        dep_qi
            .canonicalize()
            .expect("dependency artifact path should canonicalize"),
    );
    assert!(references[1..].iter().all(|location| location.uri == uri));
}

#[test]
fn package_bridge_surfaces_dependency_variant_references_without_semantic_analysis() {
    let temp = TempDir::new("ql-lsp-package-variant-broken-references");
    let app_root = temp.path().join("workspace").join("app");
    let app_path = temp
        .path()
        .join("workspace")
        .join("app")
        .join("src")
        .join("lib.ql");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    let dep_qi = temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub enum Command {
    Retry,
    Stop,
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    let source = r#"
package demo.app

use demo.dep.Command as Cmd

pub fn current(flag: Bool) -> Int {
    let command = if flag { Cmd.Retry } else { Cmd.Stop }
    match command {
        Cmd.Retry => 1,
        Cmd.Stop => 0,
    }
    return "oops"
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");

    let with_declaration = references_for_dependency_variants(
        &uri,
        source,
        &package,
        offset_to_position(source, nth_offset(source, "Retry", 1)),
        true,
    )
    .expect("dependency variant references should exist even without semantic analysis");
    assert_eq!(with_declaration.len(), 3);
    assert_eq!(
        with_declaration[0]
            .uri
            .to_file_path()
            .expect("definition URI should convert to a file path")
            .canonicalize()
            .expect("definition path should canonicalize"),
        dep_qi
            .canonicalize()
            .expect("dependency artifact path should canonicalize"),
    );
    assert!(
        with_declaration[1..]
            .iter()
            .all(|location| location.uri == uri)
    );

    let without_declaration = references_for_dependency_variants(
        &uri,
        source,
        &package,
        offset_to_position(source, nth_offset(source, "Retry", 2)),
        false,
    )
    .expect("dependency variant references should exist without declaration");
    assert_eq!(without_declaration.len(), 2);
    assert!(
        without_declaration
            .iter()
            .all(|location| location.uri == uri)
    );
}

#[test]
fn package_bridge_surfaces_dependency_struct_field_references_without_semantic_analysis() {
    let temp = TempDir::new("ql-lsp-package-struct-field-broken-references");
    let app_root = temp.path().join("workspace").join("app");
    let app_path = temp
        .path()
        .join("workspace")
        .join("app")
        .join("src")
        .join("lib.ql");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    let dep_qi = temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Config {
    value: Int,
    limit: Int,
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    let source = r#"
package demo.app

use demo.dep.Config as Cfg

pub fn read(config: Cfg) -> Int {
    let built = Cfg { value: 1, limit: 2 }
    match config {
        Cfg { value: current, limit: 3 } => current,
    }
    return "oops"
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");

    let with_declaration = references_for_dependency_struct_fields(
        &uri,
        source,
        &package,
        offset_to_position(source, nth_offset(source, "value", 1)),
        true,
    )
    .expect("dependency struct field references should exist even without semantic analysis");
    assert_eq!(with_declaration.len(), 3);
    assert_eq!(
        with_declaration[0]
            .uri
            .to_file_path()
            .expect("definition URI should convert to a file path")
            .canonicalize()
            .expect("definition path should canonicalize"),
        dep_qi
            .canonicalize()
            .expect("dependency artifact path should canonicalize"),
    );
    assert!(
        with_declaration[1..]
            .iter()
            .all(|location| location.uri == uri)
    );

    let without_declaration = references_for_dependency_struct_fields(
        &uri,
        source,
        &package,
        offset_to_position(source, nth_offset(source, "value", 2)),
        false,
    )
    .expect("dependency struct field references should exist without declaration");
    assert_eq!(without_declaration.len(), 2);
    assert!(
        without_declaration
            .iter()
            .all(|location| location.uri == uri)
    );
}

#[test]
fn package_bridge_surfaces_dependency_struct_field_shorthand_queries_without_semantic_analysis() {
    let temp = TempDir::new("ql-lsp-package-struct-field-shorthand-broken-query");
    let app_root = temp.path().join("workspace").join("app");
    let app_path = temp
        .path()
        .join("workspace")
        .join("app")
        .join("src")
        .join("lib.ql");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    let dep_qi = temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Config {
    value: Int,
    limit: Int,
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    let source = r#"
package demo.app

use demo.dep.Config as Cfg

pub fn read(value: Int, config: Cfg) -> Int {
    let built = Cfg { value, limit: 2 }
    match config {
        Cfg { value, limit: 3 } => value,
    }
    return "oops"
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should succeed");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");
    let literal_field = nth_offset(source, "value", 2);
    let pattern_field = nth_offset(source, "value", 3);

    let hover = hover_for_dependency_struct_fields(
        source,
        &package,
        offset_to_position(source, literal_field),
    )
    .expect("dependency shorthand field hover should exist even without semantic analysis");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(markup.value.contains("**field** `value`"));
    assert!(markup.value.contains("field value: Int"));

    let definition = definition_for_dependency_struct_fields(
        source,
        &package,
        offset_to_position(source, pattern_field),
    )
    .expect("dependency shorthand field definition should exist even without semantic analysis");
    let GotoDefinitionResponse::Scalar(Location {
        uri: definition_uri,
        range,
    }) = definition
    else {
        panic!("definition should be one location")
    };
    assert_eq!(
        definition_uri
            .to_file_path()
            .expect("definition URI should convert to a file path")
            .canonicalize()
            .expect("definition path should canonicalize"),
        dep_qi
            .canonicalize()
            .expect("dependency artifact path should canonicalize"),
    );
    let artifact = fs::read_to_string(&dep_qi)
        .expect("dependency interface artifact should exist")
        .replace("\r\n", "\n");
    let snippet = "value";
    let start = artifact
        .find(snippet)
        .expect("field name should exist in dependency artifact");
    assert_eq!(
        range,
        span_to_range(&artifact, Span::new(start, start + snippet.len()))
    );

    let with_declaration = references_for_dependency_struct_fields(
        &uri,
        source,
        &package,
        offset_to_position(source, literal_field),
        true,
    )
    .expect("dependency shorthand field references should exist even without semantic analysis");
    assert_eq!(with_declaration.len(), 3);
    assert_eq!(
        with_declaration[0]
            .uri
            .to_file_path()
            .expect("definition URI should convert to a file path")
            .canonicalize()
            .expect("definition path should canonicalize"),
        dep_qi
            .canonicalize()
            .expect("dependency artifact path should canonicalize"),
    );
    assert!(
        with_declaration[1..]
            .iter()
            .all(|location| location.uri == uri)
    );
}

#[test]
fn package_bridge_surfaces_dependency_references() {
    let temp = TempDir::new("ql-lsp-package-refs");
    let app_root = temp.path().join("workspace").join("app");
    let app_path = temp
        .path()
        .join("workspace")
        .join("app")
        .join("src")
        .join("lib.ql");

    let dep_qi = temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub fn exported(value: Int) -> Int
"#,
    );
    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    let source = r#"
package demo.app

use demo.dep.exported as run

pub fn main() -> Int {
    return run(1) + run(2)
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("source should analyze");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");

    let with_declaration = references_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        offset_to_position(source, nth_offset(source, "run", 2)),
        true,
    )
    .expect("dependency references should exist");
    assert_eq!(with_declaration.len(), 4);
    assert_eq!(
        with_declaration[0]
            .uri
            .to_file_path()
            .expect("definition URI should convert to a file path")
            .canonicalize()
            .expect("definition path should canonicalize"),
        dep_qi
            .canonicalize()
            .expect("dependency artifact path should canonicalize"),
    );

    let artifact = fs::read_to_string(&dep_qi)
        .expect("dependency interface artifact should exist")
        .replace("\r\n", "\n");
    let snippet = "fn exported(value: Int) -> Int";
    let start = artifact
        .find(snippet)
        .expect("exported signature should exist");
    assert_eq!(
        with_declaration[0].range,
        span_to_range(&artifact, Span::new(start, start + snippet.len()))
    );

    let without_declaration = references_for_package_analysis(
        &uri,
        source,
        &analysis,
        &package,
        offset_to_position(source, nth_offset(source, "run", 2)),
        false,
    )
    .expect("dependency references should exist without declaration");
    assert_eq!(without_declaration.len(), 2);
    assert!(
        without_declaration
            .iter()
            .all(|location| location.uri == uri)
    );
    assert_eq!(
        without_declaration[0].range,
        span_to_range(
            source,
            Span::new(
                nth_offset(source, "run", 2),
                nth_offset(source, "run", 2) + "run".len(),
            ),
        )
    );
    assert_eq!(
        without_declaration[1].range,
        span_to_range(
            source,
            Span::new(
                nth_offset(source, "run", 3),
                nth_offset(source, "run", 3) + "run".len(),
            ),
        )
    );
}
