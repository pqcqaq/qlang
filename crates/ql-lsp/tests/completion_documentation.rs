use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use ql_analysis::{analyze_package, analyze_source};
use ql_lsp::bridge::{completion_for_analysis, completion_for_package_analysis};
use tower_lsp::lsp_types::{CompletionResponse, Documentation, MarkupKind, Position};

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
    Position::new(line, prefix[line_start..].chars().count() as u32)
}

#[test]
fn completion_bridge_surfaces_markdown_documentation_for_same_file_items() {
    let source = r#"
fn helper(value: Int) -> Int {
    value
}

fn main() -> Int {
    hel
}
"#;
    let analysis = analyze_source(source).expect("source should analyze");

    let Some(CompletionResponse::Array(items)) = completion_for_analysis(
        source,
        &analysis,
        offset_to_position(source, nth_offset(source, "hel", 1) + "hel".len()),
    ) else {
        panic!("completion should exist")
    };

    let helper = items
        .into_iter()
        .find(|item| item.label == "helper")
        .expect("helper completion should exist");
    let Some(Documentation::MarkupContent(markup)) = helper.documentation else {
        panic!("completion documentation should use markdown")
    };
    assert_eq!(markup.kind, MarkupKind::Markdown);
    assert!(markup.value.contains("fn helper(value: Int) -> Int"));
}

#[test]
fn completion_bridge_surfaces_markdown_documentation_for_dependency_items() {
    let temp = TempDir::new("ql-lsp-completion-documentation");
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

pub fn main() -> Int {
    let built = Cfg { fl: true }
    1
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(source).expect("source should analyze");

    let Some(CompletionResponse::Array(items)) = completion_for_package_analysis(
        source,
        &analysis,
        &package,
        offset_to_position(source, nth_offset(source, "fl", 1) + "fl".len()),
    ) else {
        panic!("dependency completion should exist")
    };

    let field = items
        .into_iter()
        .find(|item| item.label == "flag")
        .expect("field completion should exist");
    let Some(Documentation::MarkupContent(markup)) = field.documentation else {
        panic!("dependency completion documentation should use markdown")
    };
    assert_eq!(markup.kind, MarkupKind::Markdown);
    assert!(markup.value.contains("field flag: Bool"));
    assert!(markup.value.contains("Type: `Bool`"));
}
