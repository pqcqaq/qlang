use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use ql_analysis::{analyze_package, analyze_package_dependencies, analyze_source};
use ql_lsp::bridge::{completion_for_dependency_member_fields, completion_for_package_analysis};
use tower_lsp::lsp_types::{CompletionItemKind, CompletionResponse, Position};

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

fn dependency_qi() -> &'static str {
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

pub fn maybe_children() -> Option[[Child; 2]]
"#
}

fn build_source(broken: bool) -> &'static str {
    if broken {
        r#"
package demo.app

use demo.dep.{maybe_children as kids}

pub fn read() -> Int {
    let value = kids()?[0].lea
    return "oops"
}
"#
    } else {
        r#"
package demo.app

use demo.dep.{maybe_children as kids}

pub fn read() -> Int {
    let value = kids()?[0].lea
    return 0
}
"#
    }
}

fn run_completion_case(broken: bool) {
    let temp = TempDir::new(&format!(
        "ql-lsp-grouped-question-indexed-iterable-field-member-completion{}",
        if broken { "-broken" } else { "" }
    ));
    let app_root = temp.path().join("workspace").join("app");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write("workspace/dep/dep.qi", dependency_qi());
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    let source = build_source(broken);
    temp.write("workspace/app/src/lib.ql", source);

    let position = offset_to_position(source, nth_offset(source, ".lea", 1) + ".lea".len());
    let completion = if broken {
        assert!(analyze_package(&app_root).is_err());
        let package = analyze_package_dependencies(&app_root)
            .expect("dependency-only package analysis should succeed");
        completion_for_dependency_member_fields(source, &package, position)
    } else {
        let package = analyze_package(&app_root).expect("package analysis should succeed");
        let analysis = analyze_source(source).expect("analysis should succeed for completion");
        completion_for_package_analysis(source, &analysis, &package, position)
    };

    let Some(CompletionResponse::Array(items)) = completion else {
        panic!(
            "grouped question indexed iterable field member completion should exist{}",
            if broken {
                " without semantic analysis"
            } else {
                ""
            }
        );
    };

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "leaf");
    assert_eq!(items[0].kind, Some(CompletionItemKind::FIELD));
    assert_eq!(items[0].detail.as_deref(), Some("field leaf: Leaf"));
}

#[test]
fn completion_works_on_grouped_question_indexed_iterable_field_members() {
    run_completion_case(false);
}

#[test]
fn completion_fallback_works_on_grouped_question_indexed_iterable_field_members() {
    run_completion_case(true);
}
