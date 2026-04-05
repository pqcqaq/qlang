use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use ql_analysis::{analyze_package, analyze_source};
use ql_lsp::bridge::{
    completion_for_package_analysis, definition_for_package_analysis, hover_for_package_analysis,
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
