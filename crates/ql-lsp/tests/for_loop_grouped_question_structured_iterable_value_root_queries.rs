use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use ql_analysis::{analyze_package, analyze_source};
use ql_lsp::bridge::{
    declaration_for_package_analysis, definition_for_package_analysis, hover_for_package_analysis,
    references_for_package_analysis, span_to_range,
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
enum RootKind {
    Function,
    Static,
}

impl RootKind {
    fn label(self) -> &'static str {
        match self {
            Self::Function => "function",
            Self::Static => "static",
        }
    }

    fn dep_struct_snippet(self) -> &'static str {
        "pub struct Child {\n    value: Int,\n}"
    }

    fn dependency_qi(self) -> &'static str {
        match self {
            Self::Function => {
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
            Self::Static => {
                r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Child {
    value: Int,
}

pub static MAYBE_ITEMS: Option[[Child; 2]]

impl Child {
    pub fn get(self) -> Int
}
"#
            }
        }
    }

    fn use_decl(self) -> &'static str {
        match self {
            Self::Function => "use demo.dep.{maybe_children as kids}",
            Self::Static => "use demo.dep.{MAYBE_ITEMS as maybe_items}",
        }
    }

    fn receiver_expr(self) -> &'static str {
        match self {
            Self::Function => "kids()?",
            Self::Static => "maybe_items?",
        }
    }

    fn body_expr(self) -> &'static str {
        match self {
            Self::Function => "current.value + current.value",
            Self::Static => "current.get() + current.get()",
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
            Self::Match => format!(
                "match flag {{\n        true => {expr},\n        false => {expr},\n    }}"
            ),
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

fn build_source(root: RootKind, structured: StructuredKind) -> String {
    format!(
        r#"
package demo.app

{use_decl}

pub fn read(flag: Bool) -> Int {{
    for current in ({receiver}) {{
        return {body}
    }}
    return 0
}}
"#,
        use_decl = root.use_decl(),
        receiver = structured.wrap(root.receiver_expr()),
        body = root.body_expr(),
    )
}

fn run_root_query_case(root: RootKind, structured: StructuredKind) {
    let temp = TempDir::new(&format!(
        "ql-lsp-for-loop-grouped-question-structured-iterable-{}-{}-value-root-query",
        root.label(),
        structured.label()
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
    let dep_qi = temp.write("workspace/dep/dep.qi", root.dependency_qi());
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    let source = build_source(root, structured);
    temp.write("workspace/app/src/lib.ql", &source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let analysis = analyze_source(&source).expect("source should analyze");
    let uri = Url::from_file_path(&app_path).expect("app path should convert to file URL");
    let current_usage = nth_offset(&source, "current", 2);

    let hover = hover_for_package_analysis(
        &source,
        &analysis,
        &package,
        offset_to_position(&source, current_usage),
    )
    .expect("grouped structured dependency question iterable value root hover should exist");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("hover should use markdown")
    };
    assert!(markup.value.contains("**struct** `Child`"));
    assert!(markup.value.contains("struct Child"));

    let definition = definition_for_package_analysis(
        &uri,
        &source,
        &analysis,
        &package,
        offset_to_position(&source, current_usage),
    )
    .expect("grouped structured dependency question iterable value root definition should exist");
    let GotoDefinitionResponse::Scalar(location) = definition else {
        panic!("definition should be one location")
    };
    assert_dependency_location(&location, &dep_qi, root.dep_struct_snippet());

    let declaration = declaration_for_package_analysis(
        &uri,
        &source,
        &analysis,
        &package,
        offset_to_position(&source, current_usage),
    )
    .expect("grouped structured dependency question iterable value root declaration should exist");
    let GotoDeclarationResponse::Scalar(location) = declaration else {
        panic!("declaration should be one location")
    };
    assert_dependency_location(&location, &dep_qi, root.dep_struct_snippet());

    let without_declaration = references_for_package_analysis(
        &uri,
        &source,
        &analysis,
        &package,
        offset_to_position(&source, current_usage),
        false,
    )
    .expect("grouped structured dependency question iterable value root references should exist");
    assert_eq!(
        without_declaration,
        vec![
            Location::new(
                uri.clone(),
                span_to_range(
                    &source,
                    Span::new(
                        nth_offset(&source, "current", 2),
                        nth_offset(&source, "current", 2) + "current".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    &source,
                    Span::new(
                        nth_offset(&source, "current", 3),
                        nth_offset(&source, "current", 3) + "current".len(),
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
        offset_to_position(&source, current_usage),
        true,
    )
    .expect(
        "grouped structured dependency question iterable value root references with declaration should exist",
    );
    assert_eq!(with_declaration.len(), 4);
    assert_dependency_location(&with_declaration[0], &dep_qi, root.dep_struct_snippet());
    assert_eq!(
        with_declaration[1..],
        [
            Location::new(
                uri.clone(),
                span_to_range(
                    &source,
                    Span::new(
                        nth_offset(&source, "current", 1),
                        nth_offset(&source, "current", 1) + "current".len(),
                    ),
                ),
            ),
            Location::new(
                uri.clone(),
                span_to_range(
                    &source,
                    Span::new(
                        nth_offset(&source, "current", 2),
                        nth_offset(&source, "current", 2) + "current".len(),
                    ),
                ),
            ),
            Location::new(
                uri,
                span_to_range(
                    &source,
                    Span::new(
                        nth_offset(&source, "current", 3),
                        nth_offset(&source, "current", 3) + "current".len(),
                    ),
                ),
            ),
        ]
    );
}

#[test]
fn root_query_bridge_surfaces_dependency_for_loop_if_grouped_question_function_bindings() {
    run_root_query_case(RootKind::Function, StructuredKind::If);
}

#[test]
fn root_query_bridge_surfaces_dependency_for_loop_match_grouped_question_function_bindings() {
    run_root_query_case(RootKind::Function, StructuredKind::Match);
}

#[test]
fn root_query_bridge_surfaces_dependency_for_loop_if_grouped_question_static_bindings() {
    run_root_query_case(RootKind::Static, StructuredKind::If);
}

#[test]
fn root_query_bridge_surfaces_dependency_for_loop_match_grouped_question_static_bindings() {
    run_root_query_case(RootKind::Static, StructuredKind::Match);
}
