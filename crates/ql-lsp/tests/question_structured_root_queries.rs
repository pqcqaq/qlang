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
use ql_span::Span;
use tower_lsp::lsp_types::request::GotoDeclarationResponse;
use tower_lsp::lsp_types::{GotoDefinitionResponse, Hover, HoverContents, Location, Position, Url};

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

#[derive(Clone, Copy)]
enum RootKind {
    Field,
    Method,
}

impl RootKind {
    fn label(self) -> &'static str {
        match self {
            Self::Field => "field",
            Self::Method => "method",
        }
    }

    fn root_expr(self) -> &'static str {
        match self {
            Self::Field => "config.child?",
            Self::Method => "config.child()?",
        }
    }

    fn dep_qi(self) -> &'static str {
        match self {
            Self::Field => {
                r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Child {
    value: Int,
}

pub struct Config {
    child: Option[Child],
}
"#
            }
            Self::Method => {
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
    id: Int,
}

impl Config {
    pub fn child(self) -> Result[Child, ErrInfo]
}
"#
            }
        }
    }
}

#[derive(Clone, Copy)]
enum StructuredKind {
    If,
    Match,
}

impl StructuredKind {
    fn label(self) -> &'static str {
        match self {
            Self::If => "if",
            Self::Match => "match",
        }
    }

    fn wrap(self, expr: &str) -> String {
        match self {
            Self::If => format!("if flag {{ {expr} }} else {{ {expr} }}"),
            Self::Match => {
                format!("match flag {{\n        true => {expr},\n        false => {expr},\n    }}")
            }
        }
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

fn assert_dependency_location(location: &Location, dep_qi: &Path, snippet: &str) {
    let artifact = fs::read_to_string(dep_qi)
        .expect("dependency interface artifact should exist")
        .replace("\r\n", "\n");
    let start = artifact
        .find(snippet)
        .expect("dependency snippet should exist");
    assert_eq!(
        location
            .uri
            .to_file_path()
            .expect("dependency URI should convert to a file path")
            .canonicalize()
            .expect("dependency URI path should canonicalize"),
        dep_qi
            .canonicalize()
            .expect("dependency artifact path should canonicalize"),
    );
    assert_eq!(
        location.range,
        span_to_range(&artifact, Span::new(start, start + snippet.len())),
    );
}

fn assert_child_hover(hover: Hover) {
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(markup.value.contains("**struct** `Child`"));
    assert!(markup.value.contains("struct Child"));
}

fn assert_root_references(
    locations: Vec<Location>,
    uri: &Url,
    source: &str,
    with_declaration: bool,
) {
    if with_declaration {
        assert_eq!(locations.len(), 3);
    } else {
        assert_eq!(locations.len(), 2);
    }
    let expected = [
        Location::new(
            uri.clone(),
            span_to_range(
                source,
                Span::new(
                    nth_offset(source, "child", 1),
                    nth_offset(source, "child", 1) + "child".len(),
                ),
            ),
        ),
        Location::new(
            uri.clone(),
            span_to_range(
                source,
                Span::new(
                    nth_offset(source, "child", 2),
                    nth_offset(source, "child", 2) + "child".len(),
                ),
            ),
        ),
    ];
    if with_declaration {
        assert_eq!(locations[1..], expected);
    } else {
        assert_eq!(locations, expected);
    }
}

fn build_source(root: RootKind, structured: StructuredKind, broken: bool) -> String {
    let broken_line = if broken {
        "    let broken: Int = \"oops\"\n"
    } else {
        ""
    };
    format!(
        r#"
package demo.app

use demo.dep.Config as Cfg

pub fn read(config: Cfg, flag: Bool) -> Int {{
{broken_line}    return ({wrapped}).value
}}
"#,
        wrapped = structured.wrap(root.root_expr()),
    )
}

fn run_root_query_case(root: RootKind, structured: StructuredKind, broken: bool) {
    let temp = TempDir::new(&format!(
        "ql-lsp-{}-question-{}-root-query{}",
        structured.label(),
        root.label(),
        if broken { "-broken" } else { "" }
    ));
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
    let dep_qi = temp.write("workspace/dep/dep.qi", root.dep_qi());
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    let source = build_source(root, structured, broken);
    temp.write("workspace/app/src/lib.ql", &source);

    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");
    let root_position = offset_to_position(&source, nth_offset(&source, "child", 1));

    if broken {
        assert!(analyze_package(&app_root).is_err());
        let package = analyze_package_dependencies(&app_root)
            .expect("dependency-only package analysis should succeed");

        let hover = hover_for_dependency_values(&source, &package, root_position)
            .expect("structured question-unwrapped dependency root hover should exist");
        assert_child_hover(hover);

        let definition = definition_for_dependency_values(&source, &package, root_position)
            .expect("structured question-unwrapped dependency root definition should exist");
        let GotoDefinitionResponse::Scalar(location) = definition else {
            panic!("definition should be one location")
        };
        assert_dependency_location(&location, &dep_qi, "pub struct Child {\n    value: Int,\n}");

        let declaration = declaration_for_dependency_values(&source, &package, root_position)
            .expect("structured question-unwrapped dependency root declaration should exist");
        let GotoDeclarationResponse::Scalar(location) = declaration else {
            panic!("declaration should be one location")
        };
        assert_dependency_location(&location, &dep_qi, "pub struct Child {\n    value: Int,\n}");

        let without_declaration =
            references_for_dependency_values(&uri, &source, &package, root_position, false)
                .expect("structured question-unwrapped dependency root references should exist");
        assert_root_references(without_declaration, &uri, &source, false);

        let with_declaration =
            references_for_dependency_values(&uri, &source, &package, root_position, true)
                .expect(
                    "structured question-unwrapped dependency root references with declaration should exist",
                );
        assert_dependency_location(
            &with_declaration[0],
            &dep_qi,
            "pub struct Child {\n    value: Int,\n}",
        );
        assert_root_references(with_declaration, &uri, &source, true);
    } else {
        let package = analyze_package(&app_root).expect("package analysis should succeed");
        let analysis = analyze_source(&source).expect("source should analyze");

        let hover = hover_for_package_analysis(&source, &analysis, &package, root_position)
            .expect("structured question-unwrapped dependency root hover should exist");
        assert_child_hover(hover);

        let definition =
            definition_for_package_analysis(&uri, &source, &analysis, &package, root_position)
                .expect("structured question-unwrapped dependency root definition should exist");
        let GotoDefinitionResponse::Scalar(location) = definition else {
            panic!("definition should be one location")
        };
        assert_dependency_location(&location, &dep_qi, "pub struct Child {\n    value: Int,\n}");

        let declaration =
            declaration_for_package_analysis(&uri, &source, &analysis, &package, root_position)
                .expect("structured question-unwrapped dependency root declaration should exist");
        let GotoDeclarationResponse::Scalar(location) = declaration else {
            panic!("declaration should be one location")
        };
        assert_dependency_location(&location, &dep_qi, "pub struct Child {\n    value: Int,\n}");

        let without_declaration = references_for_package_analysis(
            &uri,
            &source,
            &analysis,
            &package,
            root_position,
            false,
        )
        .expect("structured question-unwrapped dependency root references should exist");
        assert_root_references(without_declaration, &uri, &source, false);

        let with_declaration =
            references_for_package_analysis(&uri, &source, &analysis, &package, root_position, true)
                .expect(
                    "structured question-unwrapped dependency root references with declaration should exist",
                );
        assert_dependency_location(
            &with_declaration[0],
            &dep_qi,
            "pub struct Child {\n    value: Int,\n}",
        );
        assert_root_references(with_declaration, &uri, &source, true);
    }
}

#[test]
fn root_query_bridge_surfaces_if_structured_question_unwrapped_dependency_field_roots() {
    run_root_query_case(RootKind::Field, StructuredKind::If, false);
}

#[test]
fn root_query_fallback_surfaces_if_structured_question_unwrapped_dependency_field_roots_without_semantic_analysis()
 {
    run_root_query_case(RootKind::Field, StructuredKind::If, true);
}

#[test]
fn root_query_bridge_surfaces_match_structured_question_unwrapped_dependency_field_roots() {
    run_root_query_case(RootKind::Field, StructuredKind::Match, false);
}

#[test]
fn root_query_fallback_surfaces_match_structured_question_unwrapped_dependency_field_roots_without_semantic_analysis()
 {
    run_root_query_case(RootKind::Field, StructuredKind::Match, true);
}

#[test]
fn root_query_bridge_surfaces_if_structured_question_unwrapped_dependency_method_roots() {
    run_root_query_case(RootKind::Method, StructuredKind::If, false);
}

#[test]
fn root_query_fallback_surfaces_if_structured_question_unwrapped_dependency_method_roots_without_semantic_analysis()
 {
    run_root_query_case(RootKind::Method, StructuredKind::If, true);
}

#[test]
fn root_query_bridge_surfaces_match_structured_question_unwrapped_dependency_method_roots() {
    run_root_query_case(RootKind::Method, StructuredKind::Match, false);
}

#[test]
fn root_query_fallback_surfaces_match_structured_question_unwrapped_dependency_method_roots_without_semantic_analysis()
 {
    run_root_query_case(RootKind::Method, StructuredKind::Match, true);
}
