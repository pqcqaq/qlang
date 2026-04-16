use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use ql_analysis::{analyze_package, analyze_package_dependencies, analyze_source};
use ql_lsp::bridge::{
    declaration_for_dependency_struct_fields, declaration_for_package_analysis,
    definition_for_dependency_struct_fields, definition_for_package_analysis,
    hover_for_dependency_struct_fields, hover_for_package_analysis,
    references_for_dependency_struct_fields, references_for_package_analysis, span_to_range,
};
use ql_span::Span;
use tower_lsp::lsp_types::request::GotoDeclarationResponse;
use tower_lsp::lsp_types::{GotoDefinitionResponse, HoverContents, Location, Position, Url};

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

    fn receiver_expr(self) -> &'static str {
        match self {
            Self::If => "if flag { maybe_children()? } else { maybe_children()? }",
            Self::Match => {
                "match flag {\n        true => maybe_children()?,\n        false => maybe_children()?,\n    }"
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

fn assert_dependency_location(location: &Location, dep_qi: &Path, anchor: &str, name: &str) {
    let artifact = fs::read_to_string(dep_qi)
        .expect("dependency interface artifact should exist")
        .replace("\r\n", "\n");
    let anchor_start = artifact
        .find(anchor)
        .expect("dependency anchor should exist");
    let start = anchor_start
        + anchor
            .find(name)
            .expect("dependency name should exist inside anchor");
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
        span_to_range(&artifact, Span::new(start, start + name.len())),
    );
}

fn dependency_qi() -> &'static str {
    r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Child {
    value: Int,
}

pub fn maybe_children() -> Option[[Child; 2]]
"#
}

fn build_source(structured: StructuredKind, broken: bool) -> String {
    let tail = if broken {
        "    return \"oops\"\n"
    } else {
        "    return first + second\n"
    };
    format!(
        r#"
package demo.app

use demo.dep.maybe_children

pub fn read(flag: Bool) -> Int {{
    let first = ({receiver})[0].value
    let second = ({receiver})[1].value
{tail}}}
"#,
        receiver = structured.receiver_expr(),
        tail = tail,
    )
}

fn run_query_case(structured: StructuredKind, broken: bool) {
    let temp = TempDir::new(&format!(
        "ql-lsp-direct-structured-question-indexed-{}-receiver-field-query{}",
        structured.label(),
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
    let dep_qi = temp.write("workspace/dep/dep.qi", dependency_qi());
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    let source = build_source(structured, broken);
    temp.write("workspace/app/src/lib.ql", &source);
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");
    let first_offset = nth_offset(&source, "value", 1);
    let second_offset = nth_offset(&source, "value", 2);

    if broken {
        assert!(analyze_package(&app_root).is_err());
        let package = analyze_package_dependencies(&app_root)
            .expect("dependency-only package analysis should succeed");

        let hover = hover_for_dependency_struct_fields(
            &source,
            &package,
            offset_to_position(&source, first_offset),
        )
        .expect("direct structured question indexed receiver field hover should exist");
        let HoverContents::Markup(markup) = hover.contents else {
            panic!("hover should use markdown")
        };
        assert!(markup.value.contains("**field** `value`"));
        assert!(markup.value.contains("field value: Int"));

        let definition = definition_for_dependency_struct_fields(
            &source,
            &package,
            offset_to_position(&source, second_offset),
        )
        .expect("direct structured question indexed receiver field definition should exist");
        let GotoDefinitionResponse::Scalar(location) = definition else {
            panic!("definition should be one location")
        };
        assert_dependency_location(&location, &dep_qi, "value: Int", "value");

        let declaration = declaration_for_dependency_struct_fields(
            &source,
            &package,
            offset_to_position(&source, first_offset),
        )
        .expect("direct structured question indexed receiver field declaration should exist");
        let GotoDeclarationResponse::Scalar(location) = declaration else {
            panic!("declaration should be one location")
        };
        assert_dependency_location(&location, &dep_qi, "value: Int", "value");

        let with_declaration = references_for_dependency_struct_fields(
            &uri,
            &source,
            &package,
            offset_to_position(&source, first_offset),
            true,
        )
        .expect("direct structured question indexed receiver field references should exist");
        assert_eq!(with_declaration.len(), 3);
        assert_dependency_location(&with_declaration[0], &dep_qi, "value: Int", "value");
        assert!(with_declaration[1..]
            .iter()
            .all(|location| location.uri == uri));

        let without_declaration = references_for_dependency_struct_fields(
            &uri,
            &source,
            &package,
            offset_to_position(&source, second_offset),
            false,
        )
        .expect(
            "direct structured question indexed receiver field references without declaration should exist",
        );
        assert_eq!(without_declaration.len(), 2);
        assert!(without_declaration
            .iter()
            .all(|location| location.uri == uri));
    } else {
        let package = analyze_package(&app_root).expect("package analysis should succeed");
        let analysis = analyze_source(&source).expect("source should analyze");

        let hover = hover_for_package_analysis(
            &source,
            &analysis,
            &package,
            offset_to_position(&source, first_offset),
        )
        .expect("direct structured question indexed receiver field hover should exist");
        let HoverContents::Markup(markup) = hover.contents else {
            panic!("hover should use markdown")
        };
        assert!(markup.value.contains("**field** `value`"));
        assert!(markup.value.contains("field value: Int"));

        let definition = definition_for_package_analysis(
            &uri,
            &source,
            &analysis,
            &package,
            offset_to_position(&source, second_offset),
        )
        .expect("direct structured question indexed receiver field definition should exist");
        let GotoDefinitionResponse::Scalar(location) = definition else {
            panic!("definition should be one location")
        };
        assert_dependency_location(&location, &dep_qi, "value: Int", "value");

        let declaration = declaration_for_package_analysis(
            &uri,
            &source,
            &analysis,
            &package,
            offset_to_position(&source, first_offset),
        )
        .expect("direct structured question indexed receiver field declaration should exist");
        let GotoDeclarationResponse::Scalar(location) = declaration else {
            panic!("declaration should be one location")
        };
        assert_dependency_location(&location, &dep_qi, "value: Int", "value");

        let with_declaration = references_for_package_analysis(
            &uri,
            &source,
            &analysis,
            &package,
            offset_to_position(&source, first_offset),
            true,
        )
        .expect("direct structured question indexed receiver field references should exist");
        assert_eq!(with_declaration.len(), 3);
        assert_dependency_location(&with_declaration[0], &dep_qi, "value: Int", "value");
        assert!(with_declaration[1..]
            .iter()
            .all(|location| location.uri == uri));

        let without_declaration = references_for_package_analysis(
            &uri,
            &source,
            &analysis,
            &package,
            offset_to_position(&source, second_offset),
            false,
        )
        .expect(
            "direct structured question indexed receiver field references without declaration should exist",
        );
        assert_eq!(without_declaration.len(), 2);
        assert!(without_declaration
            .iter()
            .all(|location| location.uri == uri));
    }
}

#[test]
fn queries_work_on_if_direct_structured_question_indexed_receiver_fields() {
    run_query_case(StructuredKind::If, false);
}

#[test]
fn queries_work_on_match_direct_structured_question_indexed_receiver_fields() {
    run_query_case(StructuredKind::Match, false);
}

#[test]
fn queries_fallback_on_if_direct_structured_question_indexed_receiver_fields() {
    run_query_case(StructuredKind::If, true);
}

#[test]
fn queries_fallback_on_match_direct_structured_question_indexed_receiver_fields() {
    run_query_case(StructuredKind::Match, true);
}
