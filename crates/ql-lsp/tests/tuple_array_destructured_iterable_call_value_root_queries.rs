use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use ql_analysis::{analyze_package, analyze_package_dependencies, analyze_source};
use ql_lsp::bridge::{
    declaration_for_dependency_values, declaration_for_package_analysis,
    definition_for_dependency_values, definition_for_package_analysis, hover_for_dependency_values,
    hover_for_package_analysis, references_for_dependency_values, references_for_package_analysis,
    span_to_range,
};
use tower_lsp::lsp_types::request::GotoDeclarationResponse;
use tower_lsp::lsp_types::{GotoDefinitionResponse, HoverContents, Location, Position, Url};

#[derive(Clone, Copy)]
enum CaseKind {
    Tuple,
    Array,
}

impl CaseKind {
    const fn pattern(self) -> &'static str {
        match self {
            Self::Tuple => "(first, second)",
            Self::Array => "[first, second]",
        }
    }
}

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

fn build_source(case: CaseKind, broken: bool) -> String {
    let tail = if broken {
        "    let mirror = first.value\n    return \"oops\"".to_owned()
    } else {
        "    return first.value + second.value".to_owned()
    };
    format!(
        r#"
package demo.app

use demo.dep.Config as Cfg

pub fn read(config: Cfg) -> Int {{
    let {} = config.children()
    let left = first.value
    let right = second.value
{}
}}
"#,
        case.pattern(),
        tail,
    )
}

fn run_case(case: CaseKind, broken: bool) {
    let temp = TempDir::new("ql-lsp-destructured-iterable-call");
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

pub struct Config {}

impl Config {
    pub fn children(self) -> [Child; 2]
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
    let source = build_source(case, broken);
    temp.write("workspace/app/src/lib.ql", &source);
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");
    let first_usage = nth_offset(&source, "first", 2);

    if broken {
        assert!(analyze_package(&app_root).is_err());
        let package = analyze_package_dependencies(&app_root)
            .expect("dependency-only package analysis should succeed");
        let hover = hover_for_dependency_values(
            &source,
            &package,
            offset_to_position(&source, first_usage),
        )
        .expect("hover should exist");
        let HoverContents::Markup(markup) = hover.contents else {
            panic!("hover should use markdown")
        };
        assert!(markup.value.contains("Child"));
        let definition = definition_for_dependency_values(
            &source,
            &package,
            offset_to_position(&source, first_usage),
        )
        .expect("definition should exist");
        let GotoDefinitionResponse::Scalar(_) = definition else {
            panic!("definition should be scalar")
        };
        let declaration = declaration_for_dependency_values(
            &source,
            &package,
            offset_to_position(&source, first_usage),
        )
        .expect("declaration should exist");
        let GotoDeclarationResponse::Scalar(_) = declaration else {
            panic!("declaration should be scalar")
        };

        let without_declaration = references_for_dependency_values(
            &uri,
            &source,
            &package,
            offset_to_position(&source, first_usage),
            false,
        )
        .expect("references should exist");
        assert_eq!(
            without_declaration,
            vec![
                Location::new(
                    uri.clone(),
                    span_to_range(
                        &source,
                        ql_span::Span::new(
                            nth_offset(&source, "first", 2),
                            nth_offset(&source, "first", 2) + "first".len(),
                        ),
                    ),
                ),
                Location::new(
                    uri.clone(),
                    span_to_range(
                        &source,
                        ql_span::Span::new(
                            nth_offset(&source, "first", 3),
                            nth_offset(&source, "first", 3) + "first".len(),
                        ),
                    ),
                ),
            ]
        );

        let with_declaration = references_for_dependency_values(
            &uri,
            &source,
            &package,
            offset_to_position(&source, first_usage),
            true,
        )
        .expect("references with declaration should exist");
        assert_eq!(with_declaration.len(), 4);
    } else {
        let package = analyze_package(&app_root).expect("package analysis should succeed");
        let analysis = analyze_source(&source).expect("analysis should succeed");
        let hover = hover_for_package_analysis(
            &source,
            &analysis,
            &package,
            offset_to_position(&source, first_usage),
        )
        .expect("hover should exist");
        let HoverContents::Markup(markup) = hover.contents else {
            panic!("hover should use markdown")
        };
        assert!(markup.value.contains("Child"));
        let definition = definition_for_package_analysis(
            &uri,
            &source,
            &analysis,
            &package,
            offset_to_position(&source, first_usage),
        )
        .expect("definition should exist");
        let GotoDefinitionResponse::Scalar(_) = definition else {
            panic!("definition should be scalar")
        };
        let declaration = declaration_for_package_analysis(
            &uri,
            &source,
            &analysis,
            &package,
            offset_to_position(&source, first_usage),
        )
        .expect("declaration should exist");
        let GotoDeclarationResponse::Scalar(_) = declaration else {
            panic!("declaration should be scalar")
        };

        let without_declaration = references_for_package_analysis(
            &uri,
            &source,
            &analysis,
            &package,
            offset_to_position(&source, first_usage),
            false,
        )
        .expect("references should exist");
        assert_eq!(
            without_declaration,
            vec![
                Location::new(
                    uri.clone(),
                    span_to_range(
                        &source,
                        ql_span::Span::new(
                            nth_offset(&source, "first", 2),
                            nth_offset(&source, "first", 2) + "first".len(),
                        ),
                    ),
                ),
                Location::new(
                    uri.clone(),
                    span_to_range(
                        &source,
                        ql_span::Span::new(
                            nth_offset(&source, "first", 3),
                            nth_offset(&source, "first", 3) + "first".len(),
                        ),
                    ),
                ),
            ]
        );

        let with_declaration = references_for_package_analysis(
            &uri,
            &source,
            &analysis,
            &package,
            offset_to_position(&source, first_usage),
            true,
        )
        .expect("references with declaration should exist");
        assert_eq!(with_declaration.len(), 4);
    }
}

#[test]
fn tuple_destructured_dependency_iterable_call_value_root_queries_work() {
    run_case(CaseKind::Tuple, false);
}

#[test]
fn tuple_destructured_dependency_iterable_call_value_root_queries_work_without_semantic_analysis() {
    run_case(CaseKind::Tuple, true);
}

#[test]
fn array_destructured_dependency_iterable_call_value_root_queries_work() {
    run_case(CaseKind::Array, false);
}

#[test]
fn array_destructured_dependency_iterable_call_value_root_queries_work_without_semantic_analysis() {
    run_case(CaseKind::Array, true);
}
