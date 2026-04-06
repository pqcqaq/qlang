use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use ql_analysis::{analyze_package, analyze_package_dependencies, analyze_source};
use ql_lsp::bridge::{
    completion_for_dependency_imports, completion_for_dependency_member_fields,
    completion_for_dependency_methods, completion_for_dependency_struct_fields,
    completion_for_dependency_variants, completion_for_package_analysis,
    definition_for_dependency_imports, definition_for_dependency_methods,
    definition_for_dependency_struct_fields, definition_for_dependency_variants,
    definition_for_package_analysis, hover_for_dependency_imports, hover_for_dependency_methods,
    hover_for_dependency_struct_fields, hover_for_dependency_variants, hover_for_package_analysis,
    references_for_dependency_imports, references_for_dependency_methods,
    references_for_dependency_struct_fields, references_for_dependency_variants,
    references_for_package_analysis, span_to_range,
};
use ql_span::Span;
use tower_lsp::lsp_types::{
    CompletionItemKind, CompletionResponse, CompletionTextEdit, GotoDefinitionResponse,
    HoverContents, Location, Position, TextEdit, Url,
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

fn offset_to_position(source: &str, offset: usize) -> Position {
    let prefix = &source[..offset];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() as u32;
    let line_start = prefix.rfind('\n').map(|index| index + 1).unwrap_or(0);
    Position::new(line, (prefix[line_start..].chars().count()) as u32)
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
    assert_eq!(items[0].kind, Some(CompletionItemKind::FUNCTION));
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
    assert_eq!(items[0].kind, Some(CompletionItemKind::FUNCTION));
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
