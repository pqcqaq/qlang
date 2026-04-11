use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use ql_analysis::{analyze_package, analyze_package_dependencies, analyze_source};
use ql_lsp::bridge::{
    span_to_range, type_definition_for_dependency_values, type_definition_for_package_analysis,
};
use tower_lsp::lsp_types::request::GotoTypeDefinitionResponse;
use tower_lsp::lsp_types::{Location, Position, Url};

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

fn assert_targets_dependency_type(
    definition: GotoTypeDefinitionResponse,
    dep_qi: &Path,
    snippet: &str,
) {
    let GotoTypeDefinitionResponse::Scalar(Location {
        uri: definition_uri,
        range,
    }) = definition
    else {
        panic!("type definition should be one location")
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

    let artifact = fs::read_to_string(dep_qi)
        .expect("dependency interface artifact should exist")
        .replace("\r\n", "\n");
    let type_def = artifact
        .find(snippet)
        .expect("type signature should exist in dependency artifact");
    assert_eq!(
        range,
        span_to_range(
            &artifact,
            ql_span::Span::new(type_def, type_def + snippet.len())
        )
    );
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

fn run_type_definition_case(root: RootKind, structured: StructuredKind, broken: bool) {
    let temp = TempDir::new(&format!(
        "ql-lsp-{}-question-{}-type-definition{}",
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
    let position = offset_to_position(&source, nth_offset(&source, "child", 1));

    if broken {
        assert!(analyze_package(&app_root).is_err());
        let package = analyze_package_dependencies(&app_root)
            .expect("dependency-only package analysis should succeed");
        let definition = type_definition_for_dependency_values(&source, &package, position)
            .expect("structured question-unwrapped root type definition should exist");
        assert_targets_dependency_type(
            definition,
            &dep_qi,
            "pub struct Child {\n    value: Int,\n}",
        );
    } else {
        let package = analyze_package(&app_root).expect("package analysis should succeed");
        let analysis = analyze_source(&source).expect("source should analyze");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");
        let definition =
            type_definition_for_package_analysis(&uri, &source, &analysis, &package, position)
                .expect("structured question-unwrapped root type definition should exist");
        assert_targets_dependency_type(
            definition,
            &dep_qi,
            "pub struct Child {\n    value: Int,\n}",
        );
    }
}

#[test]
fn type_definition_bridge_follows_if_structured_question_unwrapped_dependency_field_types() {
    run_type_definition_case(RootKind::Field, StructuredKind::If, false);
}

#[test]
fn type_definition_fallback_follows_if_structured_question_unwrapped_dependency_field_types() {
    run_type_definition_case(RootKind::Field, StructuredKind::If, true);
}

#[test]
fn type_definition_bridge_follows_match_structured_question_unwrapped_dependency_field_types() {
    run_type_definition_case(RootKind::Field, StructuredKind::Match, false);
}

#[test]
fn type_definition_fallback_follows_match_structured_question_unwrapped_dependency_field_types() {
    run_type_definition_case(RootKind::Field, StructuredKind::Match, true);
}

#[test]
fn type_definition_bridge_follows_if_structured_question_unwrapped_dependency_method_return_types()
{
    run_type_definition_case(RootKind::Method, StructuredKind::If, false);
}

#[test]
fn type_definition_fallback_follows_if_structured_question_unwrapped_dependency_method_return_types()
 {
    run_type_definition_case(RootKind::Method, StructuredKind::If, true);
}

#[test]
fn type_definition_bridge_follows_match_structured_question_unwrapped_dependency_method_return_types()
 {
    run_type_definition_case(RootKind::Method, StructuredKind::Match, false);
}

#[test]
fn type_definition_fallback_follows_match_structured_question_unwrapped_dependency_method_return_types()
 {
    run_type_definition_case(RootKind::Method, StructuredKind::Match, true);
}
